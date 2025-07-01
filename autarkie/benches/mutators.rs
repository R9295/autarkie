use autarkie::fuzzer::context::Context;
use autarkie::fuzzer::mutators::splice::AutarkieSpliceMutator;
use autarkie::{impl_hash, impl_input, DepthInfo};
use autarkie::{Node, Visitor};
use blake3::Hash;
use criterion::{criterion_group, criterion_main, Criterion};
use libafl::{
    corpus::InMemoryCorpus,
    inputs::BytesInput,
    mutators::{MutationResult, Mutator},
    state::{HasCorpus, HasRand, StdState},
    HasMetadata,
};
use libafl_bolts::rands::StdRand;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::{cell::RefCell, rc::Rc};
use tempdir::TempDir;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, autarkie::Grammar)]
pub struct Data {
    list: Vec<DataEnum>,
    v_of_v: Vec<Vec<(u16, u32, u64,)>>,
    ints: (u8, u16, u32, u64, u128, i8, i16, i32, i64, i128),
    string: String,
    hashmap: BTreeMap<u64, Vec<u16>>,
    recursive: Box<Data>,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, autarkie::Grammar)]
enum DataEnum {
    Who,
    Ami(String),
    Int(u64, i32),
    Recurse(Box<Data>),
}

impl_hash!(Data);
impl_input!(Data);

fn benchmark_splice_mutator(c: &mut Criterion) {
    let generate_depth = DepthInfo {
        generate: 2,
        iterate: 5,
    };
    let mut visitor = Visitor::new(0, generate_depth);
    Data::__autarkie_register(&mut visitor, None, 0);
    visitor.calculate_recursion();
    let visitor = Rc::new(RefCell::new(visitor));
    let mut mutator = AutarkieSpliceMutator::<Data>::new(Rc::clone(&visitor), 100);
    let temp = TempDir::new("autarkie").unwrap();
    let mut state = StdState::new(
        StdRand::with_seed(0),
        InMemoryCorpus::<Data>::new(),
        InMemoryCorpus::new(),
        &mut (),
        &mut (),
    )
    .unwrap();

    state.add_metadata(Context::new(temp.path().to_path_buf(), false));
    let mut generated = Vec::with_capacity(5);
    while generated.len() != 5 {
        let Some(mut input) = Data::__autarkie_generate(&mut visitor.borrow_mut(), &mut 0, 0)
        else {
            continue;
        };
        generated.push(input);
    }
    c.bench_function("splice_mutator", |b| {
        b.iter(|| {
            for mut i in generated.clone() {
                let mutated = mutator.mutate(&mut state, &mut i);
            }
        });
    });
}

criterion_group!(benches, benchmark_splice_mutator);
criterion_main!(benches);
