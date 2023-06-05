use std::{cell::RefCell, path::PathBuf, rc::Rc};

use crate::file_store::{FileNode, FileViewNode};

pub struct PlayList {
    pub file_list: Vec<Rc<RefCell<FileNode>>>,
}

impl PlayList {

    pub fn new() -> PlayList {
        PlayList {
            file_list: vec![]
        }
    }

    pub fn skip(&mut self) {
        if !self.file_list.is_empty() {
            self.file_list.remove(0);
        }
    }

    pub fn current(&self) -> Option<Rc<RefCell<FileNode>>> {
        if self.file_list.is_empty() {
            None
        } else {
            Some(Rc::clone(&self.file_list[0]))
        }
    }

    pub fn add(&mut self, node: &Rc<RefCell<FileNode>>) {
        self.file_list.push(Rc::clone(node));
    }

    pub fn add_fileviewnode(&mut self, node: &Rc<RefCell<FileViewNode>>) {
        let n = node.borrow();
        &self.file_list.push(Rc::clone(&n.file_node()));
    }
    
}
