use failure::Fallible;
use log::*;
use serde::{Deserialize, Serialize};

use std::ffi::OsString;
use std::iter::IntoIterator;

use structopt::StructOpt;
fn is_default<T: Default + PartialEq>(t: &T) -> bool {
    t == &T::default()
}

// ugly, but serde and structopt use different methods to define defaults
macro_rules! set_default {
    ($name:ident, $val:expr, $type:ty) => {
        paste::item! {
            fn [<def_ $name>]() -> $type {
                $val
            }
            fn [<def_ $name _if>](e: &$type) -> bool {
                e == &[<def_ $name>]()
            }
        }
    };
}

set_default!(cache_compression_level, 12, u32);
set_default!(cache_max_blob_len, 2000000, u32);
set_default!(max_archive_recursion, 4, i32);

#[derive(StructOpt, Debug, Deserialize, Serialize)]
#[structopt(rename_all = "kebab-case", set_term_width = 80)]
pub struct RgaArgs {
    #[serde(default, skip_serializing_if = "is_default")]
    #[structopt(long = "--rga-no-cache", help = "Disable caching of results")]
    pub no_cache: bool,

    #[serde(default, skip_serializing_if = "is_default")]
    #[structopt(
        long = "--rga-accurate",
        help = "Use more accurate but slower matching by mime type"
    )]
    pub accurate: bool,

    #[serde(default, skip_serializing_if = "is_default")]
    #[structopt(
        long = "--rga-adapters",
        require_equals = true,
        require_delimiter = true,
        help = "Change which adapters to use and in which priority order (descending)"
    )]
    pub adapters: Vec<String>,

    #[serde(
        default = "def_cache_max_blob_len",
        skip_serializing_if = "def_cache_max_blob_len_if"
    )]
    #[structopt(
        long = "--rga-cache-max-blob-len",
        default_value = "2000000",
        help = "Max compressed size to cache",
        long_help = "Longest byte length (after compression) to store in cache. Longer adapter outputs will not be cached and recomputed every time."
    )]
    pub cache_max_blob_len: u32,

    #[serde(
        default = "def_cache_compression_level",
        skip_serializing_if = "def_cache_compression_level_if"
    )]
    #[structopt(
        long = "--rga-cache-compression-level",
        default_value = "12",
        require_equals = true,
        help = "ZSTD compression level to apply to adapter outputs before storing in cache db"
    )]
    pub cache_compression_level: u32,

    #[serde(
        default = "def_max_archive_recursion",
        skip_serializing_if = "def_max_archive_recursion_if"
    )]
    #[structopt(
        long = "--rga-max-archive-recursion",
        default_value = "4",
        require_equals = true,
        help = "Maximum nestedness of archives to recurse into"
    )]
    pub max_archive_recursion: i32,

    // these arguments stop the process, so don't serialize them
    #[serde(skip)]
    #[structopt(long = "--rga-list-adapters", help = "List all known adapters")]
    pub list_adapters: bool,

    #[serde(skip)]
    #[structopt(long, help = "Show help for ripgrep itself")]
    pub rg_help: bool,

    #[serde(skip)]
    #[structopt(long, help = "Show version of ripgrep itself")]
    pub rg_version: bool,
}

static RGA_CONFIG: &str = "RGA_CONFIG";

pub fn parse_args<I>(args: I) -> Fallible<RgaArgs>
where
    I: IntoIterator,
    I::Item: Into<OsString> + Clone,
{
    match std::env::var(RGA_CONFIG) {
        Ok(val) => {
            debug!(
                "Loading args from env {}={}, ignoring cmd args",
                RGA_CONFIG, val
            );
            Ok(serde_json::from_str(&val)?)
        }
        Err(_) => {
            let matches = RgaArgs::from_iter(args);
            let serialized_config = serde_json::to_string(&matches)?;
            std::env::set_var(RGA_CONFIG, &serialized_config);
            debug!("{}={}", RGA_CONFIG, serialized_config);

            Ok(matches)
        }
    }
}

/// Split arguments into the ones we care about and the ones rg cares about
pub fn split_args() -> Fallible<(RgaArgs, Vec<OsString>)> {
    let mut app = RgaArgs::clap();

    app.p.create_help_and_version();
    let mut firstarg = true;
    // debug!("{:#?}", app.p.flags);
    let (our_args, mut passthrough_args): (Vec<OsString>, Vec<OsString>) = std::env::args_os()
        .partition(|os_arg| {
            if firstarg {
                // hacky, but .enumerate() would be ugly because partition is too simplistic
                firstarg = false;
                return true;
            }
            if let Some(arg) = os_arg.to_str() {
                arg.starts_with("--rga-")
                    || arg.starts_with("--rg-")
                    || arg == "--help"
                    || arg == "-h"
                    || arg == "--version"
                    || arg == "-V"
            } else {
                // args that are not unicode can only be filenames, pass them to rg
                false
            }
        });
    debug!("our_args: {:?}", our_args);
    let matches = parse_args(our_args)?;
    if matches.rg_help {
        passthrough_args.insert(0, "--help".into());
    }
    if matches.rg_version {
        passthrough_args.insert(0, "--version".into());
    }
    debug!("passthrough_args: {:?}", passthrough_args);
    Ok((matches, passthrough_args))
}
