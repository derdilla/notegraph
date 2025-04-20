use std::fs::{self, DirEntry};

use convert_case::Casing;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Model {
    nodes: Vec<Node>,
}

impl Model {
    pub fn read(dir: &str) -> Result<Self, OpenModelError> {
        let dir = fs::read_dir(dir)
            .map_err(|e| OpenModelError::NotADir)?;
        let mut files = vec![];
        // todo: consider async
        for file in dir {
            if let Some(node) = Node::parse(file.ok()) {
                files.push(node);
            } else {
                return Err(OpenModelError::CantParseNode);
            }
        }
        Ok(Model {
            nodes: files,
        })
    }

    pub fn get_nodes(&self) -> Vec<Node> {
        self.nodes.clone()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Node {
    id: String,
    title: String,
    short: String,
    long: String,
}

impl Node {
    fn parse(file: Option<DirEntry>) -> Option<Self> {
        let file = file?;
        let title_snake_case = file.file_name().to_str()?.to_string();
        let content = fs::read_to_string(file.path()).ok()?;
        let (short, long) = content
            .split_once("\n")
            .unwrap_or((content.as_str(), ""));
        Some(Node {
            title: title_snake_case.to_case(convert_case::Case::Title),
            id: title_snake_case,
            short: short.trim().to_string(),
            long: long.trim().to_string(),
        })
    } 
}

#[derive(Debug)]
pub enum OpenModelError {
    NotADir,
    CantParseNode,
}