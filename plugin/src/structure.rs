use crate::host::{State, StructureType};
use crate::plugins::main;
use crate::plugins::main::imports::{self};
use crate::plugins::main::types::{
    EitherStructure, Embedding, EmbeddingDb, EmbeddingModel, Model, Node, NumberParameters, Page,
    SequenceParameters, Structure, ThenStructure, UnsignedRange,
};
use floneumin::floneumin_language::context::document::Document;
use floneumin::floneumin_language::floneumin_sample::structured::StructuredSampler;
use floneumin::floneumin_language::floneumin_sample::structured_parser::StructureParser;
use floneumin::floneumin_language::local::{Bert, LocalSession, Mistral, Phi};
use floneumin::floneumin_language::model::{Model as _, *};
use floneumin::floneumin_language::vector_db::VectorDB;
use once_cell::sync::Lazy;
use reqwest::header::{HeaderMap, HeaderName};
use slab::Slab;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};
use wasmtime::component::Linker;
use wasmtime::component::__internal::async_trait;
use wasmtime::Config;
use wasmtime::Engine;
use wasmtime_wasi::preview2::{self, DirPerms, FilePerms, WasiView};
use wasmtime_wasi::Dir;

impl State {
    pub(crate) fn get_full_structured_parser(
        &self,
        structure: &wasmtime::component::Resource<Structure>,
    ) -> Option<StructureParser> {
        match self.structures.get(structure.rep() as usize)? {
            StructureType::Num(num) => Some(StructureParser::Num {
                min: num.min,
                max: num.max,
                integer: num.integer,
            }),
            StructureType::Str(str) => Some(StructureParser::String {
                min_len: str.min,
                max_len: str.max,
            }),
            StructureType::Literal(literal) => Some(StructureParser::Literal(literal.clone())),
            StructureType::Or(or) => Some(StructureParser::Either {
                first: Box::new(self.get_full_structured_parser(&or.first)?),
                second: Box::new(self.get_full_structured_parser(&or.second)?),
            }),
            StructureType::Then(then) => Some(StructureParser::Then {
                first: Box::new(self.get_full_structured_parser(&then.first)?),
                second: Box::new(self.get_full_structured_parser(&then.second)?),
            }),
            StructureType::Sequence(sequence) => Some(StructureParser::Sequence {
                item: Box::new(self.get_full_structured_parser(&sequence.item)?),
                separator: Box::new(self.get_full_structured_parser(&sequence.separator)?),
                min_len: sequence.min_len,
                max_len: sequence.max_len,
            }),
        }
    }
}

#[async_trait]
impl main::types::HostStructure for State {
    async fn num(
        &mut self,
        num: main::types::NumberParameters,
    ) -> wasmtime::Result<wasmtime::component::Resource<Structure>> {
        let idx = self.structures.insert(StructureType::Num(num));
        Ok(wasmtime::component::Resource::new_own(idx as u32))
    }

    async fn str(
        &mut self,
        str: main::types::UnsignedRange,
    ) -> wasmtime::Result<wasmtime::component::Resource<Structure>> {
        let idx = self.structures.insert(StructureType::Str(str));
        Ok(wasmtime::component::Resource::new_own(idx as u32))
    }

    async fn literal(
        &mut self,
        literal: String,
    ) -> wasmtime::Result<wasmtime::component::Resource<Structure>> {
        let idx = self.structures.insert(StructureType::Literal(literal));
        Ok(wasmtime::component::Resource::new_own(idx as u32))
    }

    async fn or(
        &mut self,
        or: main::types::EitherStructure,
    ) -> wasmtime::Result<wasmtime::component::Resource<Structure>> {
        let idx = self.structures.insert(StructureType::Or(or));
        Ok(wasmtime::component::Resource::new_own(idx as u32))
    }

    async fn then(
        &mut self,
        then: main::types::ThenStructure,
    ) -> wasmtime::Result<wasmtime::component::Resource<Structure>> {
        let idx = self.structures.insert(StructureType::Then(then));
        Ok(wasmtime::component::Resource::new_own(idx as u32))
    }

    async fn sequence(
        &mut self,
        sequence: main::types::SequenceParameters,
    ) -> wasmtime::Result<wasmtime::component::Resource<Structure>> {
        let idx = self.structures.insert(StructureType::Sequence(sequence));
        Ok(wasmtime::component::Resource::new_own(idx as u32))
    }

    fn drop(&mut self, rep: wasmtime::component::Resource<Structure>) -> wasmtime::Result<()> {
        self.structures.remove(rep.rep() as usize);
        Ok(())
    }
}
