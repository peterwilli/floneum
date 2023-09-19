use std::ops::{Deref, Range};

use slab::Slab;

use crate::{
    embedding::{Embedding, VectorSpace},
    model::Model,
    vector_db::VectorDB,
};

pub enum ChunkStrategy {
    Paragraph,
    Sentence,
    Words(usize),
}

impl ChunkStrategy {
    pub fn chunk(&self, string: &str) -> Vec<Range<usize>> {
        match self {
            Self::Paragraph => {
                let mut chunks = Vec::new();
                let mut start = 0;
                for (i, c) in string.char_indices() {
                    if c == '\n' {
                        if !string[start..i].trim().is_empty() {
                            chunks.push(start..i);
                        }
                        start = i + 1;
                    }
                }
                chunks
            }
            Self::Sentence => {
                let mut chunks = Vec::new();
                let mut start = 0;
                for (i, c) in string.char_indices() {
                    if c == '.' {
                        if !string[start..i].trim().is_empty() {
                            chunks.push(start..i);
                        }
                        start = i + 1;
                    }
                }
                chunks
            }
            Self::Words(n) => {
                let mut chunks = Vec::new();
                let mut start = 0;
                let mut count = 0;
                for (i, c) in string.char_indices() {
                    if c == ' ' {
                        count += 1;
                        if count >= *n {
                            if !string[start..i].trim().is_empty() {
                                chunks.push(start..i);
                            }
                            start = i + 1;
                            count = 0;
                        }
                    }
                }
                chunks
            }
        }
    }
}

pub struct Document<S: VectorSpace> {
    raw: String,
    chunks: Vec<Chunk<S>>,
}

impl<S: VectorSpace> Document<S> {
    pub async fn new<M: Model<S>>(raw: String, strategy: ChunkStrategy) -> anyhow::Result<Self> {
        let ranges = strategy.chunk(&raw);
        let mut chunks = Vec::with_capacity(ranges.len());
        for range in ranges {
            let embedding = M::embed(&raw[range.clone()]).await?;
            let chunk = Chunk {
                byte_range: range,
                embedding,
            };
            chunks.push(chunk);
        }
        Ok(Self { raw, chunks })
    }
}

pub struct Chunk<S: VectorSpace> {
    byte_range: Range<usize>,
    embedding: Embedding<S>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DocumentSnippet {
    document_id: DocumentId,
    byte_range: Range<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DocumentId(usize);

pub struct DocumentDatabase<S: VectorSpace> {
    documents: Slab<String>,
    database: VectorDB<DocumentSnippet, S>,
}

impl<S: VectorSpace + Sync + Send> DocumentDatabase<S> {
    pub fn new() -> Self {
        Self {
            documents: Slab::new(),
            database: VectorDB::new(Vec::new(), Vec::new()),
        }
    }

    pub fn add(&mut self, document: Document<S>) {
        let id = self.documents.insert(document.raw);
        for chunk in document.chunks {
            let snippet = DocumentSnippet {
                document_id: DocumentId(id),
                byte_range: chunk.byte_range,
            };
            self.database.add_embedding(chunk.embedding, snippet);
        }
    }

    pub fn search(
        &self,
        embedding: Embedding<S>,
        n: usize,
    ) -> impl Iterator<Item = DocumentSnippetRef<'_>> {
        self.database
            .get_closest(embedding, n)
            .into_iter()
            .map(|snippet| {
                let document = &self.documents[snippet.document_id.0];
                DocumentSnippetRef {
                    document,
                    byte_range: snippet.byte_range.clone(),
                }
            })
    }

    pub fn get_within(
        &self,
        embedding: Embedding<S>,
        distance: f32,
    ) -> impl Iterator<Item = DocumentSnippetRef<'_>> {
        self.database
            .get_within(embedding, distance)
            .into_iter()
            .map(|snippet| {
                let document = &self.documents[snippet.document_id.0];
                DocumentSnippetRef {
                    document,
                    byte_range: snippet.byte_range.clone(),
                }
            })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DocumentSnippetRef<'a> {
    document: &'a str,
    byte_range: Range<usize>,
}

impl DocumentSnippetRef<'_> {
    pub fn document(&self) -> &str {
        self.document
    }

    pub fn byte_range(&self) -> Range<usize> {
        self.byte_range.clone()
    }
}

impl Deref for DocumentSnippetRef<'_> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.document[self.byte_range.clone()]
    }
}
