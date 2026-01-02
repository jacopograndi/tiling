#!/usr/bin/env -S cargo +nightly -Zscript --quiet

use std::{
    env, io,
    process::{Command, Stdio},
};

const OUT_FOLDER: &str = "out";

fn main() -> Result<(), Error> {
    // get platforms to package from command line arguments
    let args = Args::read()?;
    let platforms = if let Some(p) = args.args.iter().find_map(|arg| match arg {
        Arg::Platforms(p) => Some(p),
        _ => None,
    }) {
        p.clone()
    } else {
        // defaults to all platforms
        Platform::all()
    };

    for platform in platforms {
        package_platform(&args, &platform)?;
    }

    Ok(())
}

fn package_platform(args: &Args, platform: &Platform) -> Result<(), Error> {
    let v: Verbosity = args.into();
    let crate_name = get_crate_name();
    let package = format!("{}-{}", get_crate_name(), platform.to_string());

    println!("packaging: {}", package);

    // basically cargo run --release
    platform.build(&v)?;

    // remove old root dir
    run_silent_fail(&v, &["rm", "-r", &format!("./{}/{}", OUT_FOLDER, package)])?;

    // make package root directory
    run(
        &v,
        &["mkdir", "-p", &format!("./{}/{}/", OUT_FOLDER, package)],
    )?;

    // copy executable
    run(
        &v,
        &[
            "cp",
            "-r",
            &platform.path_to_executable(),
            &format!("./{}/{}/", OUT_FOLDER, package),
        ],
    )?;

    // copy new assets
    if platform.needs_copy_assets() {
        run(
            &v,
            &[
                "cp",
                "-r",
                "assets",
                &format!("./{}/{}/assets/", OUT_FOLDER, package),
            ],
        )?;
    }

    // some platforms may want to copy other stuff from ./resources to the package
    platform.copy_resources(&v)?;

    // optional zipping
    let package_path = platform.package_post_process(&v)?;

    // if `-i` is passed, push to itch the package
    if args.check(&Arg::ItchDeploy) {
        run(
            &v,
            &[
                "butler",
                "push",
                &package_path,
                &format!("zjikra/{}:{}", crate_name, platform.to_string()),
            ],
        )?;
    }

    println!(" packaged: {}", package);

    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
enum Platform {
    Windows,
    Wasm,
    Linux,
    Android,
}

impl ToString for Platform {
    fn to_string(&self) -> String {
        match self {
            Self::Windows => "windows",
            Self::Wasm => "wasm",
            Self::Linux => "linux",
            Self::Android => "android",
        }
        .to_string()
    }
}

impl Platform {
    fn all() -> Vec<Platform> {
        vec![
            Platform::Windows,
            Platform::Wasm,
            Platform::Linux,
            Platform::Android,
        ]
    }

    fn from_str(s: &str) -> Result<Self, Error> {
        match s {
            "windows" => Ok(Self::Windows),
            "wasm" => Ok(Self::Wasm),
            "linux" => Ok(Self::Linux),
            "android" => Ok(Self::Android),
            _ => Err(format!("unknown platform {}", s).into()),
        }
    }

    fn target(&self) -> Option<&'static str> {
        match self {
            Self::Windows => Some("x86_64-pc-windows-gnu"),
            Self::Wasm => Some("wasm32-unknown-unknown"),
            Self::Linux => Some("x86_64-unknown-linux-gnu"),
            Self::Android => None,
        }
    }

    fn build(&self, v: &Verbosity) -> Result<(), Error> {
        match self {
            Self::Windows => {
                let target = self.target().unwrap();
                run(
                    &v,
                    &["cargo", "+stable", "build", "--release", "--target", target],
                )
            }
            Self::Wasm => {
                let target = self.target().unwrap();
                run(
                    &v,
                    &["cargo", "+stable", "build", "--release", "--target", target],
                )
            }
            Self::Linux => {
                let target = self.target().unwrap();
                // using nigthly for faster compilation with `-Zshare-generics=y`
                run(&v, &["cargo", "build", "--release", "--target", target])
            }
            Self::Android => run_env(
                &v,
                &["cargo", "+stable", "quad-apk", "build", "--release"],
                &[
                    ("ANDROID_HOME", "/home/j/.android-dev"),
                    ("NDK_HOME", "/home/j/.android-dev/android-ndk-r25"),
                ],
            ),
        }
    }

    fn path_to_executable(&self) -> String {
        match self {
            Self::Windows => {
                let target = self.target().unwrap();
                format!("target/{target}/release/{}.exe", get_crate_name())
            }
            Self::Wasm => {
                let target = self.target().unwrap();
                format!("target/{target}/release/{}.wasm", get_crate_name())
            }
            Self::Linux => {
                let target = self.target().unwrap();
                format!("target/{target}/release/{}", get_crate_name())
            }
            Self::Android => {
                format!(
                    "target/android-artifacts/release/apk/{}.apk",
                    get_crate_name()
                )
            }
        }
    }

    fn needs_copy_assets(&self) -> bool {
        match self {
            Self::Android => false,
            _ => true,
        }
    }

    fn copy_resources(&self, v: &Verbosity) -> Result<(), Error> {
        match self {
            Self::Wasm => {
                let package = format!("{}-{}", get_crate_name(), self.to_string());
                let out_path = &format!("./{}/{}/index.html", OUT_FOLDER, package);
                run(
                    &v,
                    &[
                        "cp",
                        "-r",
                        &format!("resources/wasm-res/index.html"),
                        out_path,
                    ],
                )?;
                run(
                    &v,
                    &[
                        "sed",
                        "-i",
                        &format!("s/CRATENAME/{}/g", get_crate_name()),
                        out_path,
                    ],
                )
            }
            _ => Ok(()),
        }
    }

    fn package_post_process(&self, v: &Verbosity) -> Result<String, Error> {
        let package = format!("{}-{}", get_crate_name(), self.to_string());
        match self {
            Self::Android => Ok(format!("{}/{}.apk", package, get_crate_name())),
            _ => {
                env::set_current_dir(OUT_FOLDER)?;
                let zipped_path = format!("{}.zip", package);
                run_silent_fail(&v, &["rm", &zipped_path])?;
                run(&v, &["zip", "-r", &zipped_path, &format!("{}", package)])?;
                env::set_current_dir("..")?;
                Ok(format!("{}/{}", OUT_FOLDER, zipped_path))
            }
        }
    }
}

fn run(v: &Verbosity, args: &[&str]) -> Result<(), Error> {
    run_complete(v, args, &[], false)
}

fn run_env(v: &Verbosity, args: &[&str], envs: &[(&str, &str)]) -> Result<(), Error> {
    run_complete(v, args, envs, false)
}

fn run_silent_fail(v: &Verbosity, args: &[&str]) -> Result<(), Error> {
    run_complete(v, args, &[], true)
}

fn run_complete(
    verbosity: &Verbosity,
    args: &[&str],
    envs: &[(&str, &str)],
    silent_fail: bool,
) -> Result<(), Error> {
    let mut stdout = Stdio::piped();
    let mut stderr = Stdio::piped();
    if verbosity == &Verbosity::Verbose {
        println!(
            "running command:{}",
            args.iter()
                .fold(String::new(), |acc, i| format!("{} {}", acc, i))
        );
        stdout = Stdio::inherit();
        stderr = Stdio::inherit();
    }
    let child = Command::new(args[0])
        .args(&args[1..])
        .envs(envs.into_iter().cloned())
        .stdout(stdout)
        .stderr(stderr)
        .spawn()?;
    let output = child.wait_with_output()?;
    if verbosity == &Verbosity::Silent && !silent_fail {
        if !output.status.success() {
            println!(
                "command failed:{}",
                args.iter()
                    .fold(String::new(), |acc, i| format!("{} {}", acc, i))
            );
            let out_stdout = String::from_utf8_lossy(output.stdout.as_slice());
            if !out_stdout.is_empty() {
                println!("stdout: {}", out_stdout);
            }
            let out_stderr = String::from_utf8_lossy(output.stderr.as_slice());
            if !out_stderr.is_empty() {
                println!("stderr: {}", out_stderr);
            }
        }
    }
    if output.status.success() || silent_fail {
        Ok(())
    } else {
        Err(format!("command failed").into())
    }
}

fn get_crate_name() -> String {
    // could also read it from a Cargo.toml
    env::current_dir()
        .expect("no current dir")
        .file_name()
        .expect("no file name")
        .to_str()
        .expect("failed to turn dir name to string")
        .to_string()
}

#[derive(Clone, Debug)]
struct Args {
    args: Vec<Arg>,
}

impl Args {
    fn read() -> Result<Self, Error> {
        let string_args: Vec<String> = env::args().collect();
        let mut args: Vec<Arg> = vec![];
        let mut prev: Option<Arg> = None;
        let iter = string_args.iter();
        // skip the executable file name
        let mut iter = iter.skip(1);
        // read args and parse the next if required
        while let Some(string_arg) = iter.next() {
            if let Some(mut prev) = prev.take() {
                prev.parse(string_arg)?;
                args.push(prev);
            } else {
                let arg = Arg::from_str(string_arg)?;
                if arg.require_next() {
                    prev = Some(arg);
                } else {
                    args.push(arg);
                }
            }
        }
        Ok(Self { args })
    }

    fn check(&self, arg: &Arg) -> bool {
        self.args.contains(arg)
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Arg {
    ItchDeploy,
    Verbose,
    Platforms(Vec<Platform>),
}

impl Arg {
    fn from_str(s: &String) -> Result<Self, Error> {
        Arg::all()
            .into_iter()
            .find(|arg| arg.representations().iter().any(|r| s == r))
            .ok_or(format!("invalid arg {}", s).into())
    }

    fn parse(&mut self, s: &String) -> Result<(), Error> {
        match self {
            Self::Platforms(platforms) => {
                for token in s.split(',') {
                    let p = Platform::from_str(token)?;
                    platforms.push(p);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn representations(&self) -> Vec<String> {
        match self {
            Self::ItchDeploy => vec![format!("-i"), format!("--itch-deploy")],
            Self::Platforms(_) => vec![format!("-p"), format!("--platforms")],
            Self::Verbose => vec![format!("-v"), format!("--verbose")],
        }
    }

    fn require_next(&self) -> bool {
        match self {
            Self::Platforms(_) => true,
            _ => false,
        }
    }

    fn all() -> Vec<Arg> {
        vec![Arg::ItchDeploy, Arg::Verbose, Arg::Platforms(vec![])]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Verbosity {
    Verbose,
    Silent,
}

impl From<&Args> for Verbosity {
    fn from(args: &Args) -> Self {
        if args.check(&Arg::Verbose) {
            Self::Verbose
        } else {
            Self::Silent
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)] // `Debug` is needed for io::Error but not used in this program
enum Error {
    Str(String),
    Io(io::Error),
}

impl From<io::Error> for Error {
    fn from(o: io::Error) -> Self {
        Error::Io(o)
    }
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Error::Str(s)
    }
}
