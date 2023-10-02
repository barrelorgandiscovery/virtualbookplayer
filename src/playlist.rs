use std::{cell::RefCell, error::Error, fs, hash::Hash, path::PathBuf, rc::Rc};

use player::FileInformations;

use crate::file_store::{FileNode, FileViewNode};

pub struct PlayList {
    pub file_list: Vec<PlaylistElement>,
}

#[derive(Clone)]
pub struct PlaylistElement {
    pub name: String,
    pub path: PathBuf,
    pub additional_informations: Option<FileInformations>,
}

/// hash implementation for playlist element
impl Hash for PlaylistElement {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.path.hash(state);
    }
    fn hash_slice<H: std::hash::Hasher>(data: &[Self], state: &mut H)
    where
        Self: Sized,
    {
        for playlist_element in data {
            playlist_element.name.hash(state);
            playlist_element.path.hash(state);
        }
    }
}

impl From<&PathBuf> for PlaylistElement {
    fn from(value: &PathBuf) -> Self {
        let mut name: String = String::new();
        if let Some(s) = value.file_name() {
            let name_duplicate = &s.to_string_lossy();
            name = name_duplicate.to_string();
        }

        PlaylistElement {
            name,
            path: value.clone(),
            additional_informations: None,
        }
    }
}

impl PlayList {
    pub fn new() -> PlayList {
        PlayList { file_list: vec![] }
    }

    pub fn skip(&mut self) {
        if !self.file_list.is_empty() {
            self.file_list.remove(0);
        }
    }

    pub fn current(&self) -> Option<PlaylistElement> {
        if self.file_list.is_empty() {
            None
        } else {
            Some(self.file_list[0].clone())
        }
    }

    #[allow(dead_code)]
    pub fn add(&mut self, node: &Rc<RefCell<FileNode>>) {
        let cell = node.borrow();
        self.add_from_path(&cell.path);
    }

    pub fn add_from_path_and_expand_playlists(&mut self, path: &PathBuf) {
        let extension_result = path.extension();

        if let Some(ext) = extension_result {
            if ext == "playlist" {
                if let Ok(result) = load(path) {
                    self.file_list.extend(result.file_list);
                }
            } else {
                self.add_from_path(path);
            }
        } else {
            self.add_from_path(path);
        }
    }

    pub fn add_from_path(&mut self, path: &PathBuf) {
        let playlist_element: PlaylistElement = path.into();
        self.file_list.push(playlist_element);
    }

    pub fn add_fileviewnode_and_read_playlists(&mut self, node: &Rc<RefCell<FileViewNode>>) {
        let filenode = node.borrow();
        let path = &filenode.node.borrow().path;
        self.add_from_path_and_expand_playlists(path);
    }
}

pub fn save(p: &PlayList, filepath: &PathBuf) -> Result<(), Box<dyn Error>> {
    let content = p
        .file_list
        .iter()
        .map(|f| f.path.clone())
        .fold(String::new(), |s, f| {
            s + "\n" + &f.as_os_str().to_string_lossy()
        });

    fs::write(filepath, content)?;

    Ok(())
}

pub fn load(filepath: &PathBuf) -> Result<PlayList, Box<dyn Error>> {
    let contents = fs::read_to_string(filepath)?;

    let file_list: Vec<PlaylistElement> = contents
        .split('\n')
        .filter(|e| !e.is_empty())
        .map(|p| (&PathBuf::from(p)).into())
        .collect();

    Ok(PlayList { file_list })
}

#[test]
pub fn test_pllaylist() {}
