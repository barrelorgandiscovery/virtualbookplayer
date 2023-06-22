use std::cell::RefCell;
use std::error::Error;
use std::fs::metadata;
use std::rc::Weak;

use log::{debug, error};
use std::fmt::{Debug, Display};
use std::{path::PathBuf, rc::Rc};

#[derive(Debug, Clone)]
pub struct FileStoreError {
    message: String,
}

impl FileStoreError {
    pub fn new(message: &str) -> FileStoreError {
        Self {
            message: message.into(),
        }
    }
}

impl Display for FileStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("file store error : {}", &self.message))
    }
}

impl std::error::Error for FileStoreError {
    fn description(&self) -> &str {
        &self.message
    }
}

pub struct FileNode {
    pub name: String,
    pub path: PathBuf,
    pub is_folder: bool,
    pub parent_folder: Option<Weak<RefCell<FileNode>>>,
    pub folder_files: Vec<Rc<RefCell<FileNode>>>,
}

impl Display for FileNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FileNode {} : isfolder ? {} , path ? {:?}, childs: {}",
            &self.name.as_str(),
            &self.is_folder,
            &self.path,
            &self.folder_files.len()
        )
    }
}

impl Debug for FileNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FileNode {} : isfolder ? {:?} , path ? {:?} , childs: {} -> [",
            &self.name.as_str(),
            &self.is_folder,
            &self.path,
            &self.folder_files.len()
        )
        .expect("error in formatting");
        for i in &self.folder_files {
            write!(f, "{:?}", &i).expect("error in formatting");
        }
        write!(f, "]")
    }
}

impl FileNode {
    pub fn new(path: &PathBuf) -> Result<FileNode, FileStoreError> {
        match path.file_name() {
            Some(filename) => {
                let md = metadata(path).unwrap();
                let is_dir = md.is_dir();

                let n = FileNode {
                    name: filename.to_str().unwrap().into(),
                    path: path.clone(),
                    is_folder: is_dir,
                    folder_files: vec![],
                    parent_folder: None,
                };
                Ok(n)
            }
            None => Err(FileStoreError::new(
                format!("Filename {:?} not found", &path).as_str(),
            )),
        }
    }
    pub fn set_parent(&mut self, parent: &Option<Rc<RefCell<FileNode>>>) {
        match parent {
            None => self.parent_folder = None,
            Some(p) => self.parent_folder = Some(Rc::downgrade(p)),
        }
    }
    pub fn folder(&self) -> bool {
        self.is_folder
    }

    #[allow(unused)]
    pub fn accept(&self, visitor: &dyn Visitor) {
        visitor.visit(self);
    }
}

pub trait Visitor {
    fn visit(&self, node: &FileNode);
}

#[derive(Debug)]
pub struct FileStore {
    pub base_path: PathBuf,
    pub root: Rc<RefCell<FileNode>>,
    pub default_view: Option<FileView>,
}

impl FileStore {
    fn recurse_construct(
        path: &PathBuf,
        parent: &Option<Rc<RefCell<FileNode>>>,
    ) -> Result<Rc<RefCell<FileNode>>, Box<dyn Error>> {
        debug!("constructing for {:?}", &path);
        let file_node_result = FileNode::new(path);
        debug!("file node constructed : {:?}", &file_node_result);
        match file_node_result {
            Ok(file_node) => {
                let r_file_node = Rc::new(RefCell::new(file_node));
                let mut childs: Vec<Rc<RefCell<FileNode>>> = Vec::new();
                {
                    let mut bn = r_file_node.borrow_mut();
                    if bn.folder() {
                        debug!("path :{:?}", &path);
                        for r in path.read_dir().expect("error opening dir") {
                            match r {
                                Ok(dir_entry) => {
                                    debug!("entry : {:?}", &dir_entry);
                                    let p = dir_entry.path();
                                    if let Ok(child) = FileStore::recurse_construct(
                                        &p,
                                        &Some(Rc::clone(&r_file_node)),
                                    ) {
                                        childs.push(child);
                                    } else {
                                        error!("error in getting {:?}", &p);
                                    }
                                }
                                Err(e) => {
                                    error!("error getting dir entry : {}", e);
                                }
                            }
                        }

                        childs.sort_by(|a, b| {
                            let ab = a.borrow();
                            let bb = b.borrow();
                            ab.name.partial_cmp(&bb.name).unwrap()
                        });

                        bn.folder_files = childs;
                        bn.set_parent(parent);
                    }
                }

                Ok(r_file_node)
            }
            Err(e) => Err(Box::new(e)),
        }
    }

    pub fn new(path: &PathBuf) -> Result<Option<FileStore>, FileStoreError> {
        if let Some(p) = path.parent() {
            let pathbuf = p.to_path_buf();

            if let Ok(data_root) = Self::recurse_construct(path, &None) {
                let mut fs = FileStore {
                    base_path: pathbuf,
                    root: data_root,
                    default_view: None,
                };

                fs.default_view = Some(fs.view(&None).unwrap());

                Ok(Some(fs))
            } else {
                error!("error opening path {:?}", &path);
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    #[allow(clippy::only_used_in_recursion)]
    pub fn recurse_construct_view(
        &self,
        node: &Rc<RefCell<FileNode>>,
        filter: &Option<String>,
    ) -> Option<Rc<RefCell<FileViewNode>>> {
        let bn = node.borrow();
        debug!("entering {:?}", node);
        if !bn.is_folder {
            // this is file
            match filter {
                Some(f) => {
                    if bn.name.contains(f) {
                        debug!("match filter");
                        Some(FileViewNode::new(Rc::clone(node), vec![]))
                    } else {
                        debug!("does not correspond to filter");
                        None
                    }
                }
                None => {
                    debug!("no filter, create the view node");
                    Some(FileViewNode::new(Rc::clone(node), vec![]))
                }
            }
        } else {
            // go to sub elements
            let mut v: Vec<Rc<RefCell<FileViewNode>>> = Vec::new();
            for i in &bn.folder_files {
                let r = self.recurse_construct_view(i, filter);
                if let Some(element_found) = r {
                    v.push(element_found);
                }
            }

            // construct the folder view element, only IF there are childrens
            if v.is_empty() {
                debug!("no childs for folder, {:?}, removed", &bn);
                None
            } else {
                debug!("there are children, create the view");
                let fvn = FileViewNode::new(Rc::clone(node), v);
                Some(fvn)
            }
        }
    }

    pub fn view(&self, filter: &Option<String>) -> Result<FileView, Box<dyn Error>> {
        let selected_files = self.recurse_construct_view(&self.root, filter);
        match selected_files {
            None => Err(FileStoreError::new(
                "fail to construct view, there is no generated elements in view",
            ))?,
            Some(s) => Ok(FileView { root: s }),
        }
    }
}

#[test]
fn test_file_node() {
    let f = FileStore::recurse_construct(&PathBuf::from("/home/use/tmp/t"), &None);
    // cannot display the file node
    println!("{:?}", &f);
}

#[test]
fn test_file_store_and_view() {
    let f = FileStore::new(&PathBuf::from("/home/use/tmp/t")).unwrap();

    let fstore = f.unwrap();
    let fv1 = &fstore.view(&None).unwrap();
    println!("{:?}", &fv1);
    let fv2 = &fstore.view(&Some("hello".into())).unwrap();
    println!("{:?}", &fv2);
}

#[derive(Debug)]
pub struct FileViewNode {
    pub node: Rc<RefCell<FileNode>>,
    pub childs: Vec<Rc<RefCell<FileViewNode>>>,
    pub expanded: bool,
    pub clicked_for_open: Option<bool>,
    pub selected: bool,
}

#[allow(dead_code)]
impl FileViewNode {
    pub fn new(
        datanode: Rc<RefCell<FileNode>>,
        childs: Vec<Rc<RefCell<FileViewNode>>>,
    ) -> Rc<RefCell<FileViewNode>> {
        let fv = FileViewNode {
            node: Rc::clone(&datanode),
            childs,
            expanded: false,
            selected: false,
            clicked_for_open: None,
        };
        Rc::new(RefCell::new(fv))
    }

    /// get a new reference to the filenode
    #[allow(dead_code)]
    pub fn file_node(&self) -> Rc<RefCell<FileNode>> {
        Rc::clone(&self.node)
    }

    /// get the node name
    pub fn name(&self) -> String {
        let n = &self.node.borrow();
        n.name.clone()
    }

    pub fn expand_all(&mut self) {
        self.expand();
        for i in &self.childs {
            let f = &mut i.borrow_mut();
            f.expand_all();
        }
    }

    pub fn expand(&mut self) {
        self.expanded = true;
        self.clicked_for_open = Some(true);
    }
}

#[derive(Debug)]
pub struct FileView {
    pub root: Rc<RefCell<FileViewNode>>,
}

impl FileView {
    #[allow(dead_code)]
    pub fn expand_all(&self) {
        let e = &mut self.root.borrow_mut();
        e.expand_all();
    }

    pub fn expand(&self) {
        let e = &mut self.root.borrow_mut();
        e.expand();
    }

    fn recurse_find_first(node: &Rc<RefCell<FileViewNode>>) -> Option<Rc<RefCell<FileViewNode>>> {
        let view_node = node.borrow();
        let file_node = view_node.node.borrow();
        if !file_node.is_folder {
            return Some(Rc::clone(node));
        }

        for n in &view_node.childs {
            let result = FileView::recurse_find_first(n);
            if result.is_some() {
                return result;
            }
        }

        None
    }

    pub fn find_first_file(&self) -> Option<Rc<RefCell<FileViewNode>>> {
        FileView::recurse_find_first(&self.root)
    }
}
