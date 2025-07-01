use autarkie::fuzzer::context::Context;
use autarkie::fuzzer::mutators::recurse_mutate::AutarkieRecurseMutator;
use autarkie::fuzzer::mutators::splice::AutarkieSpliceMutator;
use autarkie::fuzzer::mutators::splice_append::AutarkieSpliceAppendMutator;
use autarkie::{impl_converter, impl_hash, impl_input, DepthInfo};
use autarkie::{Node, Visitor};
use criterion::{criterion_group, criterion_main, Criterion};
use libafl::state::HasRand;
use libafl::{
    corpus::InMemoryCorpus,
    mutators::{MutationResult, Mutator},
    state::StdState,
    HasMetadata,
};
use libafl_bolts::rands::{Rand, StdRand};
use std::collections::BTreeMap;
use std::{cell::RefCell, rc::Rc};
use tempdir::TempDir;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, autarkie::Grammar)]
pub struct Data {
    list: Vec<DataEnum>,
    v_of_v: Vec<Vec<(u16, u32, u64)>>,
    ints: (u8, u16, u32, u64, u128, i8, i16, i32, i64, i128),
    string: String,
    hashmap: BTreeMap<u64, Vec<u16>>,
    /*     recursive: Box<Data>, */
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
impl_converter!(Data);

fn benchmark_mutators(c: &mut Criterion) {
    let generate_depth = DepthInfo {
        generate: 2,
        iterate: 5,
    };
    let mut visitor = Visitor::new(0, generate_depth);
    Data::__autarkie_register(&mut visitor, None, 0);
    visitor.calculate_recursion();
    let visitor = Rc::new(RefCell::new(visitor));
    let mut splice_mutator =
        AutarkieSpliceMutator::<Data>::new(Rc::clone(&visitor), visitor.borrow().iterate_depth());
    let mut random_mutator =
        AutarkieRecurseMutator::<Data>::new(Rc::clone(&visitor), visitor.borrow().iterate_depth());
    let mut append_mutator = AutarkieSpliceAppendMutator::<Data>::new(Rc::clone(&visitor));
    let temp = TempDir::new("autarkie").unwrap();
    let fuzzer_dir = temp.path().to_path_buf();
    if !fuzzer_dir.join("chunks").exists() {
        std::fs::create_dir(fuzzer_dir.join("chunks")).unwrap();
    }
    if !fuzzer_dir.join("rendered").exists() {
        std::fs::create_dir(fuzzer_dir.join("rendered")).unwrap();
    }
    if !fuzzer_dir.join("cmps").exists() {
        std::fs::create_dir(fuzzer_dir.join("cmps")).unwrap();
    }
    let mut state = StdState::new(
        StdRand::with_seed(0),
        InMemoryCorpus::<Data>::new(),
        InMemoryCorpus::new(),
        &mut (),
        &mut (),
    )
    .unwrap();
    let generate_depth = visitor.borrow().generate_depth();
    state.add_metadata(Context::new(temp.path().to_path_buf(), false));
    let mut generated = Vec::with_capacity(1_000);
    let mut bytes_converter = FuzzDataTargetBytesConverter::new();
    while generated.len() != 1_000 {
        let Some(mut input) =
            Data::__autarkie_generate(&mut visitor.borrow_mut(), &mut generate_depth.clone(), 0)
        else {
            continue;
        };
        generated.push(input.clone());
        let metadata = state
            .metadata_mut::<Context>()
            .expect("we must have context!");
        metadata.generated_input();
        metadata.register_input(&input, &mut visitor.borrow_mut(), &mut bytes_converter);
        metadata.default_input();
    }
    c.bench_function("splice_mutator", |b| {
        b.iter(|| {
            state.rand_mut().set_seed(0);
            visitor.borrow_mut().set_seed(0);
            let mut count = 0;
            for mut i in generated.clone() {
                let mutated = splice_mutator.mutate(&mut state, &mut i);
                if matches!(mutated, Ok(MutationResult::Mutated)) {
                    count += 1;
                }
            }
        });
    });
    c.bench_function("splice_append", |b| {
        b.iter(|| {
            state.rand_mut().set_seed(0);
            visitor.borrow_mut().set_seed(0);
            let mut count = 0;
            for mut i in generated.clone() {
                let mutated = append_mutator.mutate(&mut state, &mut i);
                if matches!(mutated, Ok(MutationResult::Mutated)) {
                    count += 1;
                }
            }
        });
    });
    c.bench_function("random", |b| {
        b.iter(|| {
            state.rand_mut().set_seed(0);
            visitor.borrow_mut().set_seed(0);
            let mut count = 0;
            for mut i in generated.clone() {
                let mutated = random_mutator.mutate(&mut state, &mut i);
                if matches!(mutated, Ok(MutationResult::Mutated)) {
                    count += 1;
                }
            }
        });
    });
}

criterion_group!(benches, benchmark_mutators);
criterion_main!(benches);
