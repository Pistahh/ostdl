use std::cmp::Ordering;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::prelude::*;
use std::io::Read;
use std::path::PathBuf;

use libflate::gzip::Decoder;
use xmlrpc::{Request, Value};

use crate::api::{make_req, val_to_response, OST_API_URL};
use crate::error::{print_err, print_if_err, Error, E_INV_RESP};
use crate::hash::size_and_hash;

/// Sub data collected from the server
#[derive(Debug)]
struct Sub {
    url: String,
    score: f64,
    lang: String,
    format: String,
}

/// A vec of Sub-s
type Subs = Vec<Sub>;

/// A vec of Sub-refs
type SubRefs<'a> = Vec<&'a Sub>;

/// What subtitles to download, only the best one or all of them
#[derive(PartialEq, Clone, Copy)]
pub(crate) enum Which {
    Best,
    All,
}

/// Converts the API result into a Sub, if the result has all the data needed
fn match_to_sub(v: &Value) -> Option<Sub> {
    let data = v.as_struct()?;

    let url = data.get("SubDownloadLink").and_then(Value::as_str)?.into();

    let lang = data
        .get("SubLanguageID")
        .and_then(Value::as_str)
        .unwrap_or("nolang")
        .into();

    let score = data.get("Score").and_then(Value::as_f64).unwrap_or(0f64);

    let format = data
        .get("SubFormat")
        .and_then(Value::as_str)
        .unwrap_or("srt")
        .into();

    Some(Sub {
        url,
        score,
        lang,
        format,
    })
}

/// Searches for the subtitles for the given file / languages
fn find_subtitles(path: &OsStr, langs: &str, token: &str) -> Result<Subs, Error> {
    let (size, hash) = size_and_hash(path)?;

    let queries = Value::Array(vec![make_req(langs, size, hash)]);

    let search_resp = Request::new("SearchSubtitles")
        .arg(token)
        .arg(queries)
        .call_url(OST_API_URL)?;

    let resp = val_to_response(&search_resp)?;

    if let Value::Array(ref hits) = resp["data"] {
        let subs: Vec<Sub> = hits
            .iter()
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
fn download_to_file(url: &str, path: &OsString) -> Result<(), Error> {
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
fn download_subtitle(
    fname_base: &PathBuf,
    lang: &str,
    idx: Option<usize>,
    sub: &Sub,
) -> Result<(), Error> {
    let mut fname_os = fname_base.as_os_str().to_os_string();
    if let Some(i) = idx {
        fname_os.push(format!(".{}-{}.{}", lang, i, &sub.format));
    } else {
        fname_os.push(format!(".{}.{}", lang, &sub.format));
    }

    download_to_file(&sub.url, &fname_os)?;

    println!("{} {:2.1}", fname_os.to_string_lossy(), sub.score);

    Ok(())
}

/// Downloads the subtitles for the given file, given languages, the ones
/// that were requested (which)
pub(crate) fn download_subtitles(
    fname: &OsStr,
    langs: &str,
    which: Which,
    token: &str,
) -> Result<(), Error> {
    let subs = find_subtitles(fname, langs, token)?;

    let fname_path = PathBuf::from(&fname);
    let fname_base: PathBuf = fname_path
        .file_stem()
        .map(PathBuf::from)
        .unwrap_or_else(|| fname_path.clone());

    for lang in langs.split(',') {
        let lang_subs = get_lang(&subs, lang);
        if lang_subs.is_empty() {
            print_err(format!(
                "{}: No {} subtitles",
                &fname_path.to_string_lossy(),
                lang
            ));
        } else if which == Which::Best {
            let res = download_subtitle(&fname_base, &lang, None, &lang_subs[0]);
            print_if_err(&res);
        } else {
            for (i, sub) in lang_subs.iter().enumerate() {
                let res = download_subtitle(&fname_base, &lang, Some(i + 1), &sub);
                print_if_err(&res);
            }
        }
    }

    Ok(())
}

/// orders two scores - higher or non-NaN first.
fn score_cmp(a: &&Sub, b: &&Sub) -> Ordering {
    let a = a.score;
    let b = b.score;
    match (a.is_nan(), b.is_nan()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        _ => b.partial_cmp(&a).unwrap(),
    }
}

/// Returns the subtitles only for the given language
/// sorted (higher score first)
fn get_lang<'a>(subs: &'a Subs, lang: &str) -> SubRefs<'a> {
    let mut lang_subs: SubRefs = subs.iter().filter(|i| i.lang == lang).collect();

    lang_subs.sort_by(score_cmp);

    lang_subs
}
