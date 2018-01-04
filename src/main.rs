
extern crate reqwest;
extern crate xmlrpc;
extern crate libflate;
#[macro_use]
extern crate clap;

use std::fs::File;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io;
use std::io::prelude::*;
use std::mem;
use std::num::Wrapping;
use std::path::PathBuf;
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::borrow::Cow;
use std::borrow::Cow::Borrowed;

use xmlrpc::{Request, Value, RequestError, Fault};
use reqwest::Client;
use libflate::gzip::Decoder;
use clap::{Arg, App};

/// opensubtitles XML-RPC API entry point
const OST_API_URL: &'static str = "http:/api.opensubtitles.org/xml-rpc";

/// A commonly used Error
const E_INV_RESP: Error = Error::Ost(Borrowed("invalid xml-rpc response"));

/// Sub data collected from the server
#[derive(Debug)]
struct Sub {
    url: String,
    score: f64,
    lang: String,
    format: String,
}

/// For hashing
const CHUNKSIZE: usize = 65536;
const CHUNKSIZE_U64: u64 = CHUNKSIZE as u64;

/// All the errors that can occur
#[derive(Debug)]
enum Error {
    Io(io::Error),
    Ost(Cow<'static, str>),
    XmlRpcRequest(RequestError),
    XmlRpcFault(Fault),
    Reqwest(reqwest::Error)
}

/// What subtitles to download, only the best one or all of them
#[derive(PartialEq, Clone, Copy)]
enum Which {
    Best,
    All
}

/// Converting all sub-errors into Error.

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error::Io(e)
    }
}

impl From<&'static str> for Error {
    fn from(e: &'static str) -> Error {
        Error::Ost(e.into())
    }
}

impl From<RequestError> for Error {
    fn from(e: RequestError) -> Error {
        Error::XmlRpcRequest(e)
    }
}

impl From<Fault> for Error {
    fn from(e: Fault) -> Error {
        Error::XmlRpcFault(e)
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Error {
        Error::Reqwest(e)
    }
}

/// To simplify definitions using the XML-RPC "struct" type
type OstDataMap = BTreeMap<String, Value>;

/// Converts an XML-RPC "struct" into an optional OstDataMap
fn val_to_map(v: &Value) -> Option<&OstDataMap> {
    if let &Value::Struct(ref data) = v {
        Some(data)
    } else {
        None
    }
}

/// Converts an XML-RPC "String" into an optional &str
fn val_to_str(v: &Value) -> Option<&str> {
    if let &Value::String(ref s) = v {
        Some(s)
    } else {
        None
    }
}

/// Converts an XML-RPC "Double" into an optional f64
fn val_to_float(v: &Value) -> Option<f64> {
    if let &Value::Double(ref num) = v {
        Some(*num)
    } else {
        None
    }
}

/// Calculates the hashes for a block
fn hash_block(mut file: &File) -> Result<Wrapping<u64>, io::Error> {
    let mut buf = [0u8; CHUNKSIZE];

    file.read(&mut buf)?;

    let buf_u64: [u64; CHUNKSIZE/8] = unsafe {
        mem::transmute(buf)
    };

    let hash = buf_u64.iter()
                      .fold(Wrapping(0), |sum, &i| sum+Wrapping(i));

    Ok(hash)
}

// Calculates the file hash using the algo described at
// http://trac.opensubtitles.org/projects/opensubtitles/wiki/HashSourceCodes
pub fn size_and_hash(path: &OsStr) -> Result<(u64, u64), io::Error> {
    let mut file = File::open(path)?;
    let c1 = hash_block(&file)?;
    let fsize = file.seek(SeekFrom::End(0))?;
    let seekto = if fsize > CHUNKSIZE_U64 {
        fsize - CHUNKSIZE_U64
    } else {
        0
    };
    file.seek(SeekFrom::Start(seekto))?;
    let c2 = hash_block(&file)?;

    Ok((fsize, (Wrapping(fsize)+c1+c2).0))
}

/// Converts an XML-RPC response into an OstDatamap
fn val_to_response(v: &Value) -> Result<&OstDataMap, Error> {

    let resp = val_to_map(v).ok_or(E_INV_RESP)?;

    let status = resp.get("status")
                     .and_then(val_to_str)
                     .ok_or(E_INV_RESP)?;

    if status.starts_with("200") {
        Ok(resp)
    } else {
        Err(Error::Ost(format!("xmlrpc request failed: {}", status).into()))
    }
}

/// Creates the body of the search request
fn make_req(lang: &str, size: u64, hash: u64) -> Value
{
    let mut m = BTreeMap::new();
    m.insert("sublanguageid".into(), Value::String(lang.to_string()));
    m.insert("moviehash".into(), Value::String(format!("{:x}", hash)));
    m.insert("moviebytesize".into(), Value::String(size.to_string()));

    return Value::Struct(m);
}

/// A vec of Sub-s
type Subs = Vec<Sub>;

/// A vec of Sub-refs
type SubRefs<'a> = Vec<&'a Sub>;

/// Returns the subtitles only for the given language
/// sorted (higher score first)
fn get_lang<'a>(subs: &'a Subs, lang: &str) -> SubRefs<'a> {

    let mut lang_subs: SubRefs =
        subs.iter()
            .filter(|i| &i.lang == lang)
            .collect();

    lang_subs.sort_by(|a,b| b.score.partial_cmp(&a.score).unwrap());

    lang_subs
}

/// logs into OpenSubtitles API and returns the access token
fn get_token() -> Result<String, Error>
{
    let client = Client::new();

    let resp = Request::new("LogIn")
              .arg("")
              .arg("")
              .arg("en")
              .arg("opensubtitles-download 1.0")
              .call(&client, OST_API_URL)??;

    val_to_response(&resp)?
        .get("token")
        .and_then(val_to_str)
        .map(String::from)
        .ok_or(E_INV_RESP)
}

/// Converts the API result into a Sub, if the result has all the data needed
fn match_to_sub(v: &Value) -> Option<Sub> {
    let data = val_to_map(v)?;

    let url = data.get("SubDownloadLink")
                .and_then(val_to_str)?
                .into();

    let lang = data.get("SubLanguageID")
                    .and_then(val_to_str)
                    .unwrap_or("nolang")
                    .into();

    let score = data.get("Score")
                        .and_then(val_to_float)
                        .unwrap_or(0f64);

    let format = data.get("SubFormat")
                    .and_then(val_to_str)
                    .unwrap_or("srt")
                    .into();

    Some(Sub { url, score, lang, format })
}

/// Searches for the subtitles for the given file / languages
fn find_subtitles(path: &OsStr, langs: &str, token: &str) -> Result<Subs, Error>
{
    let (size, hash) = size_and_hash(path)?;

    let queries = Value::Array(vec!(make_req(langs, size, hash)));

    let client = Client::new();

    let search_resp =
        Request::new("SearchSubtitles")
        .arg(token)
        .arg(queries)
        .call(&client, OST_API_URL)??;

    let resp = val_to_response(&search_resp)?;

    if let Value::Array(ref hits) = resp["data"] {

        // for i in hits.iter() {
        //     if let &Value::Struct(ref hits_struct) = i {
        //         for (k, v) in hits_struct {
        //             println!(">>> {} => {:?} <<<", k, v);
        //         }
        //     }
        // }

        let subs: Vec<Sub> = hits.iter()
                                 .map(match_to_sub)
                                 .filter(Option::is_some)
                                 .map(Option::unwrap)
                                 .collect();
        Ok(subs)
    } else {
        Err(E_INV_RESP)
    }
}

/// Fetches the data from the url and gunzips it into the file
/// specified by the path
fn download_to_file(url: &str, path: &OsString) -> Result<(), Error>
{
    let mut res = reqwest::get(url)?;
    let mut file = File::create(path)?;
    let mut gzipped = Vec::new();
    res.read_to_end(&mut gzipped)?;

    let mut decoder = Decoder::new(&gzipped[..]).unwrap();
    let mut decoded_data = Vec::new();
    decoder.read_to_end(&mut decoded_data).unwrap();
    file.write_all(&decoded_data)?;

    Ok(())
}

/// Downloads the given subtitle, constructing the file name based on the
/// original filename, the language and the index
fn download_subtitle(fname_base: &PathBuf,
                     lang: &str,
                     idx: Option<usize>,
                     sub: &Sub) -> Result<(), Error>
{
    let mut fname_os = fname_base.as_os_str()
                                 .to_os_string();
    if let Some(i) = idx {
        fname_os.push(format!(".{}-{}.{}", lang, i, &sub.format));
    } else {
        fname_os.push(format!(".{}.{}", lang, &sub.format));
    }

    download_to_file(&sub.url, &fname_os)?;

    println!("{} {:2.1}",
        fname_os.to_string_lossy(),
        sub.score);

    Ok(())
}

/// Downloads the subtitles for the given file, given languages, the ones
/// that were requested (which)
fn download_subtitles(fname: &OsStr,
                      langs: &str,
                      which: Which,
                      token: &str) -> Result<(), Error>
{
    let subs = find_subtitles(fname, langs, token)?;

    let fname_path = PathBuf::from(&fname);
    let fname_base: PathBuf = fname_path.file_stem()
                              .map(PathBuf::from)
                              .unwrap_or(fname_path.clone());

    for lang in langs.split(',') {
        let lang_subs = get_lang(&subs, lang);
        if lang_subs.len() == 0 {
            print_err(format!("{}: No {} subtitles", &fname_path.to_string_lossy(), lang));
        } else if which == Which::Best {
            let res = download_subtitle(&fname_base, &lang, None, &lang_subs[0]);
            print_if_err(&res);
        } else {
            for (i, sub) in lang_subs.iter().enumerate() {
                let res = download_subtitle(&fname_base, &lang, Some(i+1), &sub);
                print_if_err(&res);
            }
        }
    }

    Ok(())
}

/// Prints an error to stderr
fn print_err(err: String)
{
    eprintln!("{}", err);
}

/// If the input is an Error then prints it to stderr
fn print_if_err<T>(res: &Result<T, Error>)
{
    if let &Err(ref err) = res {
        match err {
            &Error::Ost(ref e)            => { eprintln!("{}", e.to_string()) }
            &Error::Io(ref e)             => { eprintln!("{}", e.to_string()) }
            &Error::XmlRpcRequest(ref e)  => { eprintln!("{}", e.to_string()) }
            &Error::XmlRpcFault(ref e)    => { eprintln!("{}", e.to_string()) }
            &Error::Reqwest(ref e)        => { eprintln!("{}", e.to_string()) }
        }
    }

}

/// The real main
fn real_main() -> Result<(), Error> {
    let args = App::new("Opensubtitles downloader")
        .version(crate_version!())
        .author("Istvan Szekeres <szekeres@iii.hu>")
        .about("Downloads subtitles from opensubtitles.org")
        .arg(Arg::with_name("langs")
            .short("l")
            .long("langs")
            .help("Languages to download subtitles for, comma separated")
            .required(false)
            .takes_value(true))
        .arg(Arg::with_name("all")
            .short("a")
            .long("all")
            .help("Download all the subtitles for the selected languages")
            .required(false)
            .takes_value(false))
        .arg(Arg::with_name("FILES")
            .multiple(true)
            .required(true)
            .help("Files to download subtitles for"))
        .get_matches();

    let langs = args.value_of("langs")
                    .unwrap_or("eng");

    let which = if args.is_present("all") {
                    Which::All
                } else {
                    Which::Best
                };

    let token = get_token()?;

    if let Some(files) = args.values_of_os("FILES") {
        for fname in files {
            let res = download_subtitles(fname, &langs, which, &token);
            print_if_err(&res);
        }
    }

    Ok(())
}

/// No, the other one is the real one.
fn main() {
    let res = real_main();
    print_if_err(&res);
}
