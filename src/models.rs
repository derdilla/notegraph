use std::fs::{self, DirEntry};

use convert_case::Casing;
use once_cell::sync::Lazy;
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use regex::Regex;
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

    /// Determine all references between short and long descriptions of notes (really expensive).
    pub fn get_edges(&self) -> Vec<Vec<String>> {
        let mut res = Vec::new();
        self.nodes.par_iter()
            .map(|a| a
                .get_connections()
                .iter()
                .map(|b| vec![a.id.to_string(), b.to_string()])
                .collect::<Vec<Vec<String>>>())
            .collect_into_vec(&mut res);
        res.concat()
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

    fn get_connections(&self) -> Vec<String> {
        // TODO: ignore inside raw blocks (only run these regexes on strings ready to display)
        // [id] without (): \[([\w_\s]*)\][^\(]
        // [title](id) block: \[[^\]]*\]\(([\w\s_-]*)\)

        static ID_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\[([\w_\s]*)\][^\(]").unwrap());
        static TITLED_ID_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\[[^\]]*\]\(([\w\s_-]*)\)").unwrap());

        ID_RE.captures_iter(&self.short)
            .chain(ID_RE.captures_iter(&self.long))
            .chain(TITLED_ID_RE.captures_iter(&self.short))
            .chain(TITLED_ID_RE.captures_iter(&self.long))
            .map(|e|e.get(1).expect("1 is non-optional group in regex"))
            .map(|e| e.as_str().to_string())
            .collect()
    }
}

#[derive(Debug)]
pub enum OpenModelError {
    NotADir,
    CantParseNode,
}