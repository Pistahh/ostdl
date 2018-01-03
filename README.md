# ostdl - A downloader for opensubtitles.org

This program can be used to download subtitles from opensubtitles.org.
It calculates a hash of the input (video) file and uses that hash to search
for the subtitles created for the input video.

On successful run it prints the name of the downloaded subtitle file and its score.

## Usage

    USAGE:
        ostdl [FLAGS] [OPTIONS] [FILES]...

    FLAGS:
        -a, --all        Download all the subtitles for the selected languages
        -h, --help       Prints help information
        -V, --version    Prints version information

    OPTIONS:
        -l, --langs <langs>    Languages to download subtitles for, comma separated

    ARGS:
        <FILES>...    Files to download subtitles for

## Examples
    $ ostdl something.mkv

Downloads the best (highest score) subtitle for `something.mkv`

    $ ostdl --langs hun,spa --all *.mkv

Downloads all the hungarian and spanish subtitles for all the *.mkv files
in the current directory.

## Author

Pistahh - István Szekeres <szekeres@iii.hu>
