use criterion::{criterion_group, criterion_main, Criterion};
use sequence_map;

const ENTRIES: usize = 50000;

fn run_one(lookup: &sequence_map::Map, bits: usize, entries: usize, c: &mut Criterion) {
    c.bench_function(&format!("lookup bits={} entries={}", bits, entries), move |b| {
        b.iter(|| {
            for key in 0..entries {
                lookup.get(key as u64).expect(&format!("entry exists: {}", key));
            }
        })
    });
}

fn run_bit_size(bits: usize, entries: usize, c: &mut Criterion) {
    let mut builder = sequence_map::Builder::new(bits);
    for key in 0..entries {
        let string = format!("entry_{}", key);
        builder.insert(key as u64, &string);
    }
    let bytes = builder.build();
    let lookup = sequence_map::Map::new(&bytes);

    run_one(&lookup, bits, 1, c);
    run_one(&lookup, bits, 10, c);
    run_one(&lookup, bits, 100, c);
    run_one(&lookup, bits, 1000, c);
}

pub fn criterion_benchmark(c: &mut Criterion) {
    run_bit_size(2, ENTRIES, c);
    run_bit_size(4, ENTRIES, c);
    run_bit_size(8, ENTRIES, c);
    run_bit_size(16, ENTRIES, c);
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
