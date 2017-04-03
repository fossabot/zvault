use ::prelude::*;
use super::*;


pub enum Arguments {
    Init {
        repo_path: String,
        bundle_size: usize,
        chunker: ChunkerType,
        compression: Option<Compression>,
        encryption: bool,
        hash: HashMethod,
        remote_path: String
    },
    Backup {
        repo_path: String,
        backup_name: String,
        src_path: String,
        full: bool,
        reference: Option<String>,
        same_device: bool,
        excludes: Vec<String>,
        excludes_from: Option<String>,
        no_default_excludes: bool,
        tar: bool
    },
    Restore {
        repo_path: String,
        backup_name: String,
        inode: Option<String>,
        dst_path: String,
        tar: bool
    },
    Remove {
        repo_path: String,
        backup_name: String,
        inode: Option<String>
    },
    Prune {
        repo_path: String,
        prefix: String,
        daily: Option<usize>,
        weekly: Option<usize>,
        monthly: Option<usize>,
        yearly: Option<usize>,
        force: bool
    },
    Vacuum {
        repo_path: String,
        ratio: f32,
        force: bool
    },
    Check {
        repo_path: String,
        backup_name: Option<String>,
        inode: Option<String>,
        full: bool
    },
    List {
        repo_path: String,
        backup_name: Option<String>,
        inode: Option<String>
    },
    Info {
        repo_path: String,
        backup_name: Option<String>,
        inode: Option<String>
    },
    Mount {
        repo_path: String,
        backup_name: Option<String>,
        inode: Option<String>,
        mount_point: String
    },
    Versions {
        repo_path: String,
        path: String
    },
    Diff {
        repo_path_old: String,
        backup_name_old: String,
        inode_old: Option<String>,
        repo_path_new: String,
        backup_name_new: String,
        inode_new: Option<String>
    },
    Analyze {
        repo_path: String
    },
    BundleList {
        repo_path: String
    },
    BundleInfo {
        repo_path: String,
        bundle_id: BundleId
    },
    Import {
        repo_path: String,
        remote_path: String,
        key_files: Vec<String>
    },
    Config {
        repo_path: String,
        bundle_size: Option<usize>,
        chunker: Option<ChunkerType>,
        compression: Option<Option<Compression>>,
        encryption: Option<Option<PublicKey>>,
        hash: Option<HashMethod>
    },
    GenKey {
        file: Option<String>
    },
    AddKey {
        repo_path: String,
        file: Option<String>,
        set_default: bool
    },
    AlgoTest {
        file: String,
        bundle_size: usize,
        chunker: ChunkerType,
        compression: Option<Compression>,
        encrypt: bool,
        hash: HashMethod
    }
}


pub fn parse_repo_path(repo_path: &str, backup_restr: Option<bool>, path_restr: Option<bool>) -> Result<(&str, Option<&str>, Option<&str>), ErrorCode> {
    let mut parts = repo_path.splitn(3, "::");
    let mut repo = parts.next().unwrap_or(&DEFAULT_REPOSITORY);
    if repo.is_empty() {
        repo = &DEFAULT_REPOSITORY;
    }
    let mut backup = parts.next();
    if let Some(val) = backup {
        if val.is_empty() {
            backup = None
        }
    }
    let mut path = parts.next();
    if let Some(val) = path {
        if val.is_empty() {
            path = None
        }
    }
    if let Some(restr) = backup_restr {
        if !restr && backup.is_some() {
            error!("No backup may be given here");
            return Err(ErrorCode::InvalidArgs);
        }
        if restr && backup.is_none() {
            error!("A backup must be specified");
            return Err(ErrorCode::InvalidArgs);
        }
    }
    if let Some(restr) = path_restr {
        if !restr && path.is_some() {
            error!("No subpath may be given here");
            return Err(ErrorCode::InvalidArgs);
        }
        if restr && path.is_none() {
            error!("A subpath must be specified");
            return Err(ErrorCode::InvalidArgs);
        }
    }
    Ok((repo, backup, path))
}

fn parse_num(num: &str, name: &str) -> Result<u64, ErrorCode> {
    if let Ok(num) = num.parse::<u64>() {
        Ok(num)
    } else {
        error!("{} must be a number, was '{}'", name, num);
        Err(ErrorCode::InvalidArgs)
    }
}

fn parse_chunker(val: &str) -> Result<ChunkerType, ErrorCode> {
    if let Ok(chunker) = ChunkerType::from_string(val) {
        Ok(chunker)
    } else {
        error!("Invalid chunker method/size: {}", val);
        Err(ErrorCode::InvalidArgs)
    }
}

fn parse_compression(val: &str) -> Result<Option<Compression>, ErrorCode> {
    if val == "none" {
        return Ok(None)
    }
    if let Ok(compression) = Compression::from_string(val) {
        Ok(Some(compression))
    } else {
        error!("Invalid compression method/level: {}", val);
        Err(ErrorCode::InvalidArgs)
    }
}

fn parse_public_key(val: &str) -> Result<PublicKey, ErrorCode> {
    let bytes = match parse_hex(val) {
        Ok(bytes) => bytes,
        Err(_) => {
            error!("Invalid key: {}", val);
            return Err(ErrorCode::InvalidArgs);
        }
    };
    if let Some(key) = PublicKey::from_slice(&bytes) {
        Ok(key)
    } else {
        error!("Invalid key: {}", val);
        Err(ErrorCode::InvalidArgs)
    }
}

fn parse_hash(val: &str) -> Result<HashMethod, ErrorCode> {
    if let Ok(hash) = HashMethod::from(val) {
        Ok(hash)
    } else {
        error!("Invalid hash method: {}", val);
        Err(ErrorCode::InvalidArgs)
    }
}

fn parse_bundle_id(val: &str) -> Result<BundleId, ErrorCode> {
    if let Ok(hash) = Hash::from_string(val) {
        Ok(BundleId(hash))
    } else {
        error!("Invalid bundle id: {}", val);
        Err(ErrorCode::InvalidArgs)
    }
}

#[allow(unknown_lints,cyclomatic_complexity)]
pub fn parse() -> Result<Arguments, ErrorCode> {
    let args = clap_app!(zvault =>
        (version: crate_version!())
        (author: crate_authors!(",\n"))
        (about: crate_description!())
        (@setting SubcommandRequiredElseHelp)
        (@setting GlobalVersion)
        (@setting VersionlessSubcommands)
        (@setting UnifiedHelpMessage)
        (@subcommand init =>
            (about: "initializes a new repository")
            (@arg bundle_size: --bundlesize +takes_value "maximal bundle size in MiB [default: 25]")
            (@arg chunker: --chunker +takes_value "chunker algorithm [default: fastcdc/8]")
            (@arg compression: --compression -c +takes_value "compression to use [default: brotli/3]")
            (@arg encryption: --encryption -e "generate a keypair and enable encryption")
            (@arg hash: --hash +takes_value "hash method to use [default: blake2]")
            (@arg remote: --remote -r +takes_value +required "path to the mounted remote storage")
            (@arg REPO: "path of the repository")
        )
        (@subcommand backup =>
            (about: "creates a new backup")
            (@arg full: --full "create a full backup")
            (@arg reference: --ref +takes_value "the reference backup to use for partial backup")
            (@arg cross_device: --xdev -x "allow to cross filesystem boundaries")
            (@arg exclude: --exclude -e ... +takes_value "exclude this path or file")
            (@arg excludes_from: --excludesfrom +takes_value "read the list of exludes from this file")
            (@arg no_default_excludes: --nodefaultexcludes "do not load the default excludes file")
            (@arg tar: --tar "the source is a tar file")
            (@arg SRC: +required "source path to backup")
            (@arg BACKUP: +required "repository::backup path")
        )
        (@subcommand restore =>
            (about: "restores a backup (or subpath)")
            (@arg tar: --tar "restore in form of a tar file")
            (@arg BACKUP: +required "repository::backup[::subpath] path")
            (@arg DST: +required "destination path for backup")
        )
        (@subcommand remove =>
            (about: "removes a backup or a subpath")
            (@arg BACKUP: +required "repository::backup[::subpath] path")
        )
        (@subcommand prune =>
            (about: "removes backups based on age")
            (@arg prefix: --prefix +takes_value "only consider backups starting with this prefix")
            (@arg daily: --daily +takes_value "keep this number of daily backups")
            (@arg weekly: --weekly +takes_value "keep this number of weekly backups")
            (@arg monthly: --monthly +takes_value "keep this number of monthly backups")
            (@arg yearly: --yearly +takes_value  "keep this number of yearly backups")
            (@arg force: --force -f "actually run the prunce instead of simulating it")
            (@arg REPO: "path of the repository")
        )
        (@subcommand vacuum =>
            (about: "saves space by combining and recompressing bundles")
            (@arg ratio: --ratio -r +takes_value "ratio in % of unused space in a bundle to rewrite that bundle")
            (@arg force: --force -f "actually run the vacuum instead of simulating it")
            (@arg REPO: "path of the repository")
        )
        (@subcommand check =>
            (about: "checks the repository, a backup or a backup subpath")
            (@arg full: --full "also check file contents")
            (@arg PATH: "repository[::backup] path")
        )
        (@subcommand list =>
            (about: "lists backups or backup contents")
            (@arg PATH: "repository[::backup[::subpath]] path")
        )
        (@subcommand mount =>
            (about: "mount a backup for inspection")
            (@arg PATH: "repository[::backup[::subpath]] path")
            (@arg MOUNTPOINT: +required "where to mount to backup")
        )
        (@subcommand bundlelist =>
            (about: "lists bundles in a repository")
            (@arg REPO: "path of the repository")
        )
        (@subcommand bundleinfo =>
            (about: "lists bundles in a repository")
            (@arg REPO: "path of the repository")
            (@arg BUNDLE: +required "the bundle id")
        )
        (@subcommand import =>
            (about: "reconstruct a repository from the remote files")
            (@arg key: --key -k ... +takes_value "a file with a needed to read the bundles")
            (@arg REMOTE: +required "remote repository path")
            (@arg REPO: "path of the local repository to create")
        )
        (@subcommand info =>
            (about: "displays information on a repository, a backup or a path in a backup")
            (@arg PATH: "repository[::backup[::subpath]] path")
        )
        (@subcommand analyze =>
            (about: "analyze the used and reclaimable space of bundles")
            (@arg REPO: "repository path")
        )
        (@subcommand versions =>
            (about: "display different versions of a file in all backups")
            (@arg REPO: "repository path")
            (@arg PATH: +required "the file path")
        )
        (@subcommand diff =>
            (about: "display difference between two backup versions")
            (@arg OLD: +required "old repository::backup[::subpath] path")
            (@arg NEW: +required "new repository::backup[::subpath] path")
        )
        (@subcommand config =>
            (about: "changes the configuration")
            (@arg REPO: "path of the repository")
            (@arg bundle_size: --bundlesize +takes_value "maximal bundle size in MiB [default: 25]")
            (@arg chunker: --chunker +takes_value "chunker algorithm [default: fastcdc/16]")
            (@arg compression: --compression -c +takes_value "compression to use [default: brotli/3]")
            (@arg encryption: --encryption -e +takes_value "the public key to use for encryption")
            (@arg hash: --hash +takes_value "hash method to use [default: blake2]")
        )
        (@subcommand genkey =>
            (about: "generates a new key pair")
            (@arg FILE: +takes_value "the destination file for the keypair")
        )
        (@subcommand addkey =>
            (about: "adds a key to the respository")
            (@arg REPO: "path of the repository")
            (@arg generate: --generate -g "generate a new key")
            (@arg set_default: --default -d "set this key as default")
            (@arg FILE: +takes_value "the file containing the keypair")
        )
        (@subcommand algotest =>
            (about: "test a specific algorithm combination")
            (@arg bundle_size: --bundlesize +takes_value "maximal bundle size in MiB [default: 25]")
            (@arg chunker: --chunker +takes_value "chunker algorithm [default: fastcdc/16]")
            (@arg compression: --compression -c +takes_value "compression to use [default: brotli/3]")
            (@arg encrypt: --encrypt -e "enable encryption")
            (@arg hash: --hash +takes_value "hash method to use [default: blake2]")
            (@arg FILE: +required "the file to test the algorithms with")
        )
    ).get_matches();
    if let Some(args) = args.subcommand_matches("init") {
        let (repository, _backup, _inode) = try!(parse_repo_path(args.value_of("REPO").unwrap_or(""), Some(false), Some(false)));
        return Ok(Arguments::Init {
            bundle_size: (try!(parse_num(args.value_of("bundle_size").unwrap_or(&DEFAULT_BUNDLE_SIZE.to_string()), "Bundle size")) * 1024 * 1024) as usize,
            chunker: try!(parse_chunker(args.value_of("chunker").unwrap_or(DEFAULT_CHUNKER))),
            compression: try!(parse_compression(args.value_of("compression").unwrap_or(DEFAULT_COMPRESSION))),
            encryption: args.is_present("encryption"),
            hash: try!(parse_hash(args.value_of("hash").unwrap_or(DEFAULT_HASH))),
            repo_path: repository.to_string(),
            remote_path: args.value_of("remote").unwrap().to_string()
        })
    }
    if let Some(args) = args.subcommand_matches("backup") {
        let (repository, backup, _inode) = try!(parse_repo_path(args.value_of("BACKUP").unwrap(), Some(true), Some(false)));
        return Ok(Arguments::Backup {
            repo_path: repository.to_string(),
            backup_name: backup.unwrap().to_string(),
            full: args.is_present("full"),
            same_device: !args.is_present("cross_device"),
            excludes: args.values_of("exclude").map(|v| v.map(|k| k.to_string()).collect()).unwrap_or_else(|| vec![]),
            excludes_from: args.value_of("excludes_from").map(|v| v.to_string()),
            src_path: args.value_of("SRC").unwrap().to_string(),
            reference: args.value_of("reference").map(|v| v.to_string()),
            no_default_excludes: args.is_present("no_default_excludes"),
            tar: args.is_present("tar")
        })
    }
    if let Some(args) = args.subcommand_matches("restore") {
        let (repository, backup, inode) = try!(parse_repo_path(args.value_of("BACKUP").unwrap(), Some(true), None));
        return Ok(Arguments::Restore {
            repo_path: repository.to_string(),
            backup_name: backup.unwrap().to_string(),
            inode: inode.map(|v| v.to_string()),
            dst_path: args.value_of("DST").unwrap().to_string(),
            tar: args.is_present("tar")
        })
    }
    if let Some(args) = args.subcommand_matches("remove") {
        let (repository, backup, inode) = try!(parse_repo_path(args.value_of("BACKUP").unwrap(), Some(true), None));
        return Ok(Arguments::Remove {
            repo_path: repository.to_string(),
            backup_name: backup.unwrap().to_string(),
            inode: inode.map(|v| v.to_string())
        })
    }
    if let Some(args) = args.subcommand_matches("prune") {
        let (repository, _backup, _inode) = try!(parse_repo_path(args.value_of("REPO").unwrap_or(""), Some(false), Some(false)));
        return Ok(Arguments::Prune {
            repo_path: repository.to_string(),
            prefix: args.value_of("prefix").unwrap_or("").to_string(),
            force: args.is_present("force"),
            daily: match args.value_of("daily") {
                None => None,
                Some(v) => Some(try!(parse_num(v, "daily backups")) as usize)
            },
            weekly: match args.value_of("weekly") {
                None => None,
                Some(v) => Some(try!(parse_num(v, "weekly backups")) as usize)
            },
            monthly: match args.value_of("monthly") {
                None => None,
                Some(v) => Some(try!(parse_num(v, "monthly backups")) as usize)
            },
            yearly: match args.value_of("yearly") {
                None => None,
                Some(v) => Some(try!(parse_num(v, "yearly backups")) as usize)
            }
        })
    }
    if let Some(args) = args.subcommand_matches("vacuum") {
        let (repository, _backup, _inode) = try!(parse_repo_path(args.value_of("REPO").unwrap_or(""), Some(false), Some(false)));
        return Ok(Arguments::Vacuum {
            repo_path: repository.to_string(),
            force: args.is_present("force"),
            ratio: try!(parse_num(args.value_of("ratio").unwrap_or(&DEFAULT_VACUUM_RATIO.to_string()), "ratio")) as f32 / 100.0
        })
    }
    if let Some(args) = args.subcommand_matches("check") {
        let (repository, backup, inode) = try!(parse_repo_path(args.value_of("PATH").unwrap_or(""), None, None));
        return Ok(Arguments::Check {
            repo_path: repository.to_string(),
            backup_name: backup.map(|v| v.to_string()),
            inode: inode.map(|v| v.to_string()),
            full: args.is_present("full")
        })
    }
    if let Some(args) = args.subcommand_matches("list") {
        let (repository, backup, inode) = try!(parse_repo_path(args.value_of("PATH").unwrap_or(""), None, None));
        return Ok(Arguments::List {
            repo_path: repository.to_string(),
            backup_name: backup.map(|v| v.to_string()),
            inode: inode.map(|v| v.to_string())
        })
    }
    if let Some(args) = args.subcommand_matches("bundlelist") {
        let (repository, _backup, _inode) = try!(parse_repo_path(args.value_of("REPO").unwrap_or(""), Some(false), Some(false)));
        return Ok(Arguments::BundleList {
            repo_path: repository.to_string(),
        })
    }
    if let Some(args) = args.subcommand_matches("bundleinfo") {
        let (repository, _backup, _inode) = try!(parse_repo_path(args.value_of("REPO").unwrap_or(""), Some(false), Some(false)));
        return Ok(Arguments::BundleInfo {
            repo_path: repository.to_string(),
            bundle_id: try!(parse_bundle_id(args.value_of("BUNDLE").unwrap()))
        })
    }
    if let Some(args) = args.subcommand_matches("info") {
        let (repository, backup, inode) = try!(parse_repo_path(args.value_of("PATH").unwrap_or(""), None, None));
        return Ok(Arguments::Info {
            repo_path: repository.to_string(),
            backup_name: backup.map(|v| v.to_string()),
            inode: inode.map(|v| v.to_string())
        })
    }
    if let Some(args) = args.subcommand_matches("mount") {
        let (repository, backup, inode) = try!(parse_repo_path(args.value_of("PATH").unwrap_or(""), None, None));
        return Ok(Arguments::Mount {
            repo_path: repository.to_string(),
            backup_name: backup.map(|v| v.to_string()),
            inode: inode.map(|v| v.to_string()),
            mount_point: args.value_of("MOUNTPOINT").unwrap().to_string()
        })
    }
    if let Some(args) = args.subcommand_matches("versions") {
        let (repository, _backup, _inode) = try!(parse_repo_path(args.value_of("REPO").unwrap_or(""), Some(false), Some(false)));
        return Ok(Arguments::Versions {
            repo_path: repository.to_string(),
            path: args.value_of("PATH").unwrap().to_string()
        })
    }
    if let Some(args) = args.subcommand_matches("diff") {
        let (repository_old, backup_old, inode_old) = try!(parse_repo_path(args.value_of("OLD").unwrap(), Some(true), None));
        let (repository_new, backup_new, inode_new) = try!(parse_repo_path(args.value_of("NEW").unwrap(), Some(true), None));
        return Ok(Arguments::Diff {
            repo_path_old: repository_old.to_string(),
            backup_name_old: backup_old.unwrap().to_string(),
            inode_old: inode_old.map(|v| v.to_string()),
            repo_path_new: repository_new.to_string(),
            backup_name_new: backup_new.unwrap().to_string(),
            inode_new: inode_new.map(|v| v.to_string()),
        })
    }
    if let Some(args) = args.subcommand_matches("analyze") {
        let (repository, _backup, _inode) = try!(parse_repo_path(args.value_of("REPO").unwrap_or(""), Some(false), Some(false)));
        return Ok(Arguments::Analyze {
            repo_path: repository.to_string()
        })
    }
    if let Some(args) = args.subcommand_matches("import") {
        let (repository, _backup, _inode) = try!(parse_repo_path(args.value_of("REPO").unwrap_or(""), Some(false), Some(false)));
        return Ok(Arguments::Import {
            repo_path: repository.to_string(),
            remote_path: args.value_of("REMOTE").unwrap().to_string(),
            key_files: args.values_of("key").map(|v| v.map(|k| k.to_string()).collect()).unwrap_or_else(|| vec![])
        })
    }
    if let Some(args) = args.subcommand_matches("config") {
        let (repository, _backup, _inode) = try!(parse_repo_path(args.value_of("REPO").unwrap_or(""), Some(false), Some(false)));
        return Ok(Arguments::Config {
            bundle_size: match args.value_of("bundle_size") {
                None => None,
                Some(v) => Some((try!(parse_num(v, "Bundle size")) * 1024 * 1024) as usize)
            },
            chunker: match args.value_of("chunker") {
                None => None,
                Some(v) => Some(try!(parse_chunker(v)))
            },
            compression: match args.value_of("compression") {
                None => None,
                Some(v) => Some(try!(parse_compression(v)))
            },
            encryption: match args.value_of("encryption") {
                None => None,
                Some("none") => Some(None),
                Some(v) => Some(Some(try!(parse_public_key(v))))
            },
            hash: match args.value_of("hash") {
                None => None,
                Some(v) => Some(try!(parse_hash(v)))
            },
            repo_path: repository.to_string(),
        })
    }
    if let Some(args) = args.subcommand_matches("genkey") {
        return Ok(Arguments::GenKey {
            file: args.value_of("FILE").map(|v| v.to_string())
        })
    }
    if let Some(args) = args.subcommand_matches("addkey") {
        let (repository, _backup, _inode) = try!(parse_repo_path(args.value_of("REPO").unwrap_or(""), Some(false), Some(false)));
        let generate = args.is_present("generate");
        if !generate && !args.is_present("FILE") {
            println!("Without --generate, a file containing the key pair must be given");
            return Err(ErrorCode::InvalidArgs)
        }
        if generate && args.is_present("FILE") {
            println!("With --generate, no file may be given");
            return Err(ErrorCode::InvalidArgs)
        }
        return Ok(Arguments::AddKey {
            repo_path: repository.to_string(),
            set_default: args.is_present("set_default"),
            file: args.value_of("FILE").map(|v| v.to_string())
        })
    }
    if let Some(args) = args.subcommand_matches("algotest") {
        return Ok(Arguments::AlgoTest {
            bundle_size: (try!(parse_num(args.value_of("bundle_size").unwrap_or(&DEFAULT_BUNDLE_SIZE.to_string()), "Bundle size")) * 1024 * 1024) as usize,
            chunker: try!(parse_chunker(args.value_of("chunker").unwrap_or(DEFAULT_CHUNKER))),
            compression: try!(parse_compression(args.value_of("compression").unwrap_or(DEFAULT_COMPRESSION))),
            encrypt: args.is_present("encrypt"),
            hash: try!(parse_hash(args.value_of("hash").unwrap_or(DEFAULT_HASH))),
            file: args.value_of("FILE").unwrap().to_string(),
        })
    }
    error!("No subcommand given");
    Err(ErrorCode::InvalidArgs)
}
