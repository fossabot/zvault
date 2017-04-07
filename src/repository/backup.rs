use ::prelude::*;

use std::fs;
use std::path::{self, Path, PathBuf};
use std::collections::{HashMap, BTreeMap, VecDeque};
use std::os::linux::fs::MetadataExt;

use chrono::prelude::*;
use regex::RegexSet;


quick_error!{
    #[derive(Debug)]
    #[allow(unknown_lints,large_enum_variant)]
    pub enum BackupError {
        FailedPaths(backup: Backup, failed: Vec<PathBuf>) {
            description("Some paths could not be backed up")
            display("Backup error: some paths could not be backed up")
        }
        RemoveRoot {
            description("The root of a backup can not be removed")
            display("Backup error: the root of a backup can not be removed")
        }
    }
}


pub struct BackupOptions {
    pub same_device: bool,
    pub excludes: Option<RegexSet>
}


pub enum DiffType {
    Add, Mod, Del
}


impl Repository {
    pub fn get_backups(&self) -> Result<HashMap<String, Backup>, RepositoryError> {
        Ok(try!(Backup::get_all_from(&self.crypto.lock().unwrap(), &self.backups_path)))
    }

    pub fn get_backup(&self, name: &str) -> Result<Backup, RepositoryError> {
        Ok(try!(Backup::read_from(&self.crypto.lock().unwrap(), self.backups_path.join(name))))
    }

    pub fn save_backup(&mut self, backup: &Backup, name: &str) -> Result<(), RepositoryError> {
        let path = &self.backups_path.join(name);
        try!(fs::create_dir_all(path.parent().unwrap()));
        Ok(try!(backup.save_to(&self.crypto.lock().unwrap(), self.config.encryption.clone(), path)))
    }

    pub fn delete_backup(&self, name: &str) -> Result<(), RepositoryError> {
        let mut path = self.backups_path.join(name);
        try!(fs::remove_file(&path));
        loop {
            path = path.parent().unwrap().to_owned();
            if path == self.backups_path || fs::remove_dir(&path).is_err() {
                break
            }
        }
        Ok(())
    }


    pub fn prune_backups(&self, prefix: &str, daily: usize, weekly: usize, monthly: usize, yearly: usize, force: bool) -> Result<(), RepositoryError> {
        let mut backups = Vec::new();
        let backup_map = match self.get_backups() {
            Ok(backup_map) => backup_map,
            Err(RepositoryError::BackupFile(BackupFileError::PartialBackupsList(backup_map, _failed))) => {
                warn!("Some backups could not be read, ignoring them");
                backup_map
            },
            Err(err) => return Err(err)
        };
        for (name, backup) in backup_map {
            if name.starts_with(prefix) {
                let date = Local.timestamp(backup.date, 0);
                backups.push((name, date, backup));
            }
        }
        backups.sort_by_key(|backup| -backup.2.date);
        let mut keep = Bitmap::new(backups.len());

        fn mark_needed<K: Eq, F: Fn(&DateTime<Local>) -> K>(backups: &[(String, DateTime<Local>, Backup)], keep: &mut Bitmap, max: usize, keyfn: F) {
            let mut kept = 0;
            let mut last = None;
            for (i, backup) in backups.iter().enumerate() {
                let val = keyfn(&backup.1);
                let cur = Some(val);
                if cur != last {
                    if kept >= max {
                        break
                    }
                    last = cur;
                    keep.set(i);
                    kept += 1;
                }
            }
        }
        if yearly > 0 {
            mark_needed(&backups, &mut keep, yearly, |d| d.year());
        }
        if monthly > 0 {
            mark_needed(&backups, &mut keep, monthly, |d| (d.year(), d.month()));
        }
        if weekly > 0 {
            mark_needed(&backups, &mut keep, weekly, |d| (d.isoweekdate().0, d.isoweekdate().1));
        }
        if daily > 0 {
            mark_needed(&backups, &mut keep, daily, |d| (d.year(), d.month(), d.day()));
        }
        let mut remove = Vec::new();
        info!("Removing the following backups");
        for (i, backup) in backups.into_iter().enumerate() {
            if !keep.get(i) {
                println!("  - {}", backup.0);
                remove.push(backup.0);
            }
        }
        if force {
            for name in remove {
                try!(self.delete_backup(&name));
            }
        }
        Ok(())
    }

    pub fn restore_inode_tree<P: AsRef<Path>>(&mut self, inode: Inode, path: P) -> Result<(), RepositoryError> {
        let _lock = try!(self.lock(false));
        let mut queue = VecDeque::new();
        queue.push_back((path.as_ref().to_owned(), inode));
        while let Some((path, inode)) = queue.pop_front() {
            try!(self.save_inode_at(&inode, &path));
            if inode.file_type == FileType::Directory {
                let path = path.join(inode.name);
                for chunks in inode.children.unwrap().values() {
                    let inode = try!(self.get_inode(&chunks));
                    queue.push_back((path.clone(), inode));
                }
            }
        }
        Ok(())
    }

    pub fn create_backup_recurse<P: AsRef<Path>>(
        &mut self,
        path: P,
        reference: Option<&Inode>,
        options: &BackupOptions,
        backup: &mut Backup,
        failed_paths: &mut Vec<PathBuf>
    ) -> Result<Inode, RepositoryError> {
        let path = path.as_ref();
        let mut inode = try!(self.create_inode(path, reference));
        let meta_size = inode.estimate_meta_size();
        inode.cum_size = inode.size + meta_size;
        if let Some(ref_inode) = reference {
            if !ref_inode.is_same_meta_quick(&inode) {
                backup.changed_data_size += inode.size + meta_size;
            }
        } else {
            backup.changed_data_size += inode.size + meta_size;
        }
        if inode.file_type == FileType::Directory {
            inode.cum_dirs = 1;
            let mut children = BTreeMap::new();
            let parent_dev = try!(path.metadata()).st_dev();
            for ch in try!(fs::read_dir(path)) {
                let child = try!(ch);
                let child_path = child.path();
                if options.same_device {
                    let child_dev = try!(child.metadata()).st_dev();
                    if child_dev != parent_dev {
                        continue
                    }
                }
                if let Some(ref excludes) = options.excludes {
                    let child_path_str = child_path.to_string_lossy();
                    if excludes.is_match(&child_path_str) {
                        continue
                    }
                }
                let name = child.file_name().to_string_lossy().to_string();
                let ref_child = reference.as_ref()
                    .and_then(|inode| inode.children.as_ref())
                    .and_then(|map| map.get(&name))
                    .and_then(|chunks| self.get_inode(chunks).ok());
                let child_inode = match self.create_backup_recurse(&child_path, ref_child.as_ref(), options, backup, failed_paths) {
                    Ok(inode) => inode,
                    Err(RepositoryError::Inode(_)) | Err(RepositoryError::Chunker(_)) | Err(RepositoryError::Io(_)) => {
                        warn!("Failed to backup {:?}", child_path);
                        failed_paths.push(child_path);
                        continue
                    },
                    Err(err) => return Err(err)
                };
                let chunks = try!(self.put_inode(&child_inode));
                children.insert(name, chunks);
                inode.cum_size += child_inode.cum_size;
                inode.cum_dirs += child_inode.cum_dirs;
                inode.cum_files += child_inode.cum_files;
            }
            inode.children = Some(children);
        } else {
            inode.cum_files = 1;
        }
        Ok(inode)
    }

    #[allow(dead_code)]
    pub fn create_backup_recursively<P: AsRef<Path>>(&mut self, path: P, reference: Option<&Backup>, options: &BackupOptions) -> Result<Backup, RepositoryError> {
        let _lock = try!(self.lock(false));
        let reference_inode = reference.and_then(|b| self.get_inode(&b.root).ok());
        let mut backup = Backup::default();
        backup.config = self.config.clone();
        backup.host = get_hostname().unwrap_or_else(|_| "".to_string());
        backup.path = path.as_ref().to_string_lossy().to_string();
        let info_before = self.info();
        let start = Local::now();
        let mut failed_paths = vec![];
        let root_inode = try!(self.create_backup_recurse(path, reference_inode.as_ref(), options, &mut backup, &mut failed_paths));
        backup.root = try!(self.put_inode(&root_inode));
        try!(self.flush());
        let elapsed = Local::now().signed_duration_since(start);
        backup.date = start.timestamp();
        backup.total_data_size = root_inode.cum_size;
        backup.file_count = root_inode.cum_files;
        backup.dir_count = root_inode.cum_dirs;
        backup.duration = elapsed.num_milliseconds() as f32 / 1_000.0;
        let info_after = self.info();
        backup.deduplicated_data_size = info_after.raw_data_size - info_before.raw_data_size;
        backup.encoded_data_size = info_after.encoded_data_size - info_before.encoded_data_size;
        backup.bundle_count = info_after.bundle_count - info_before.bundle_count;
        backup.chunk_count = info_after.chunk_count - info_before.chunk_count;
        backup.avg_chunk_size = backup.deduplicated_data_size as f32 / backup.chunk_count as f32;
        if failed_paths.is_empty() {
            Ok(backup)
        } else {
            Err(BackupError::FailedPaths(backup, failed_paths).into())
        }
    }

    pub fn remove_backup_path<P: AsRef<Path>>(&mut self, backup: &mut Backup, path: P) -> Result<(), RepositoryError> {
        let _lock = try!(self.lock(false));
        let mut inodes = try!(self.get_backup_path(backup, path));
        let to_remove = inodes.pop().unwrap();
        let mut remove_from = match inodes.pop() {
            Some(inode) => inode,
            None => return Err(BackupError::RemoveRoot.into())
        };
        remove_from.children.as_mut().unwrap().remove(&to_remove.name);
        let mut last_inode_chunks = try!(self.put_inode(&remove_from));
        let mut last_inode_name = remove_from.name;
        while let Some(mut inode) = inodes.pop() {
            inode.children.as_mut().unwrap().insert(last_inode_name, last_inode_chunks);
            last_inode_chunks = try!(self.put_inode(&inode));
            last_inode_name = inode.name;
        }
        backup.root = last_inode_chunks;
        backup.modified = true;
        Ok(())
    }

    pub fn get_backup_path<P: AsRef<Path>>(&mut self, backup: &Backup, path: P) -> Result<Vec<Inode>, RepositoryError> {
        let mut inodes = vec![];
        let mut inode = try!(self.get_inode(&backup.root));
        for c in path.as_ref().components() {
            if let path::Component::Normal(name) = c {
                let name = name.to_string_lossy();
                if let Some(chunks) = inode.children.as_mut().and_then(|c| c.remove(&name as &str)) {
                    inodes.push(inode);
                    inode = try!(self.get_inode(&chunks));
                } else {
                    return Err(RepositoryError::NoSuchFileInBackup(backup.clone(), path.as_ref().to_owned()));
                }
            }
        }
        inodes.push(inode);
        Ok(inodes)
    }

    #[inline]
    pub fn get_backup_inode<P: AsRef<Path>>(&mut self, backup: &Backup, path: P) -> Result<Inode, RepositoryError> {
        self.get_backup_path(backup, path).map(|mut inodes| inodes.pop().unwrap())
    }

    #[inline]
    pub fn find_versions<P: AsRef<Path>>(&mut self, path: P) -> Result<Vec<(String, Inode)>, RepositoryError> {
        let path = path.as_ref();
        let mut versions = HashMap::new();
        for (name, backup) in try!(self.get_backups()) {
            match self.get_backup_inode(&backup, path) {
                Ok(inode) => {
                    versions.insert((inode.file_type, inode.timestamp, inode.size), (name, inode));
                },
                Err(RepositoryError::NoSuchFileInBackup(..)) => continue,
                Err(err) => return Err(err)
            }
        }
        let mut versions: Vec<_> = versions.into_iter().map(|(_, v)| v).collect();
        versions.sort_by_key(|v| v.1.timestamp);
        Ok(versions)
    }

    #[inline]
    fn find_differences_recurse(&mut self, inode1: &Inode, inode2: &Inode, path: PathBuf, diffs: &mut Vec<(DiffType, PathBuf)>) -> Result<(), RepositoryError> {
        if !inode1.is_same_meta(inode2) || inode1.data != inode2.data {
            diffs.push((DiffType::Mod, path.clone()));
        }
        if let Some(ref children1) = inode1.children {
            if let Some(ref children2) = inode2.children {
                for name in children1.keys() {
                    if !children2.contains_key(name) {
                        diffs.push((DiffType::Del, path.join(name)));
                    }
                }
            } else {
                for name in children1.keys() {
                    diffs.push((DiffType::Del, path.join(name)));
                }
            }
        }
        if let Some(ref children2) = inode2.children {
            if let Some(ref children1) = inode1.children {
                for (name, chunks2) in children2 {
                    if let Some(chunks1) = children1.get(name) {
                        if chunks1 != chunks2 {
                            let inode1 = try!(self.get_inode(chunks1));
                            let inode2 = try!(self.get_inode(chunks2));
                            try!(self.find_differences_recurse(&inode1, &inode2, path.join(name), diffs));
                        }
                    } else {
                        diffs.push((DiffType::Add, path.join(name)));
                    }
                }
            } else {
                for name in children2.keys() {
                    diffs.push((DiffType::Add, path.join(name)));
                }
            }
        }
        Ok(())
    }

    #[inline]
    pub fn find_differences(&mut self, inode1: &Inode, inode2: &Inode) -> Result<Vec<(DiffType, PathBuf)>, RepositoryError> {
        let mut diffs = vec![];
        let path = PathBuf::from("/");
        try!(self.find_differences_recurse(inode1, inode2, path, &mut diffs));
        Ok(diffs)
    }
}
