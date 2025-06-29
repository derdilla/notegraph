use std::fs::{self, DirEntry};
use std::path::PathBuf;
use std::sync::Arc;
use convert_case::{Case, Casing};
use rayon::prelude::*;
use serde::Serialize;
use typst::{
    diag::{FileError, FileResult, SourceResult},
    foundations::{Bytes, Datetime, Dict, Module, Value},
    syntax::{FileId, Source, Span, VirtualPath},
    text::{Font, FontBook, FontInfo},
    Library, World,
};
use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use system_fonts::SystemFonts;
use std::collections::HashMap;

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

// A simple world implementation for Typst that handles a single source file.
struct TypstWorld {
    library: Arc<Library>,
    source: Source,
    font_book: Arc<FontBook>,
    fonts: Vec<Font>,
}

impl TypstWorld {
    // Create a new Typst world with the given source code.
    fn new(source: &str) -> Result<Self> {
        let library = Arc::new(Library::builder().build());
        let source = Source::detached(source);
        
        let mut fonts = Vec::new();
        let mut font_book = FontBook::new();
        
        // Try to load system fonts
        if let Ok(system_fonts) = system_fonts::SystemFonts::new() {
            if let Some(fonts_by_family) = system_fonts.db().all_fonts() {
                for (_, font_paths) in fonts_by_family.iter() {
                    if let Some(path) = font_paths.first() {
                        if let Ok(data) = std::fs::read(path) {
                            if let Ok(font) = Font::new(data.into(), 0) {
                                if let Some(info) = FontInfo::new(&font) {
                                    font_book.push(info);
                                    fonts.push(font);
                                    // Just use the first available font for simplicity
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // If no fonts were found, use a built-in font
        if fonts.is_empty() {
            // Use a simple built-in font as fallback
            let font_data = include_bytes!("../assets/DejaVuSans.ttf").to_vec();
            if let Ok(font) = Font::new(font_data.into(), 0) {
                if let Some(info) = FontInfo::new(&font) {
                    font_book.push(info);
                    fonts.push(font);
                }
            }
        }
        
        // If still no fonts, return an error
        if fonts.is_empty() {
            return Err(anyhow::anyhow!("No fonts available"));
        }
        
        Ok(Self {
            library,
            source,
            font_book: Arc::new(font_book),
            fonts,
        })
    }
}

impl World for TypstWorld {
    fn library(&self) -> &Library {
        &self.library
    }

    fn book(&self) -> &FontBook {
        &self.font_book
    }

    fn main(&self) -> Source {
        self.source.clone()
    }

    fn source(&self, _id: FileId) -> FileResult<Source> {
        Ok(self.source.clone())
    }

    fn file(&self, _id: FileId) -> FileResult<Bytes> {
        Err(FileError::NotFound(PathBuf::new()))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        None
    }
}

// Renders Typst code to an SVG string.
fn render_typst_to_svg(code: &str) -> Result<String> {
    // Create a Typst world with our source code
    let world = match TypstWorld::new(code) {
        Ok(world) => world,
        Err(e) => {
            eprintln!("Failed to create Typst world: {}", e);
            return Err(e);
        }
    };
    
    // Compile the document
    let document = match typst::compile(&world) {
        Ok((document, _warnings)) => document,
        Err(errors) => {
            eprintln!("Failed to compile Typst: {:?}", errors);
            return Err(anyhow::anyhow!("Failed to compile Typst"));
        }
    };
    
    // Get the first page's frame
    let frame = document.pages.first()
        .ok_or_else(|| anyhow::anyhow!("No pages in document"))?;
    
    // Render to SVG
    let svg = typst_svg::svg(frame);
    
    // Make the SVG background transparent
    let svg = svg.replace("<rect", "<rect fill=\"none\"");
    
    Ok(svg)
}

#[derive(Debug, Clone, Serialize)]
pub struct Node {
    id: String,
    title: String,
    short: String,
    long: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    long_text_svg: Option<String>,
}

impl Node {
    fn parse(file: Option<DirEntry>) -> Option<Self> {
        let file = file?;
        let title_snake_case = file.file_name().to_str()?.to_string();
        let content = fs::read_to_string(file.path()).ok()?;
        let (short, long) = content
            .split_once("\n")
            .unwrap_or((content.as_str(), ""));
            
        let long_trimmed = long.trim();
        
        // Only try to render as Typst if the content looks like Typst code
        let long_text_svg = if long_trimmed.starts_with('#') || long_trimmed.contains('$') {
            // Wrap in a document with proper styling
            let typst_code = format!(
                r"""
                #set page(
                    width: 600pt,
                    margin: 20pt,
                    fill: rgb("transparent"),
                )
                #set text(
                    size: 12pt,
                    fill: rgb(0, 0, 0),
                )
                {}
                """,
                long_trimmed
            );
            
            match render_typst_to_svg(&typst_code) {
                Ok(svg) => Some(svg),
                Err(e) => {
                    eprintln!("Failed to render Typst to SVG: {}", e);
                    None
                }
            }
        } else {
            None
        };
        
        Some(Node {
            title: title_snake_case.to_case(Case::Title),
            id: title_snake_case,
            short: short.trim().to_string(),
            long: long_trimmed.to_string(),
            long_text_svg,
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