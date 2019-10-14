use clap::{crate_version, App, Arg, ArgMatches};

use crate::api::get_token;
use crate::error::{print_if_err, Error};
use crate::subtitle::{download_subtitles, Which};

mod api;
mod error;
mod hash;
mod subtitle;

/// The real main
fn real_main() -> Result<(), Error> {
    let args = parse_arguments();

    let langs = args.value_of("langs").unwrap_or("eng");

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

fn parse_arguments<'a>() -> ArgMatches<'a> {
    App::new("Opensubtitles downloader")
        .version(crate_version!())
        .author("Istvan Szekeres <szekeres@iii.hu>")
        .about("Downloads subtitles from opensubtitles.org")
        .arg(
            Arg::with_name("langs")
                .short("l")
                .long("langs")
                .help("Languages to download subtitles for, comma separated")
                .required(false)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("all")
                .short("a")
                .long("all")
                .help("Download all the subtitles for the selected languages")
                .required(false)
                .takes_value(false),
        )
        .arg(
            Arg::with_name("FILES")
                .multiple(true)
                .required(true)
                .help("Files to download subtitles for"),
        )
        .get_matches()
}

/// No, the other one is the real one.
fn main() {
    let res = real_main();
    print_if_err(&res);
}
