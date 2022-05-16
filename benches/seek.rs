use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::prelude::*;
use rand_pcg::Pcg64;
use stage2::physical::columns::{Column, ColumnScan, GenericColumnScan, RleColumn, VectorColumn};

pub fn benchmark_seek(c: &mut Criterion) {
    let mut rng = Pcg64::seed_from_u64(21564);
    let mut data: Vec<usize> = Vec::new();
    let _seek: Vec<usize>;
    for _i in 0..10000001 {
        data.push(rng.gen::<usize>());
    }
    data.sort_unstable();
    let randa = data[rng.gen_range(0..10000000)];

    let test_column = VectorColumn::new(data.clone());
    let rle_test_column = RleColumn::new(data.clone());

    let mut group = c.benchmark_group("seek");
    group.sample_size(200);
    group.bench_function("seek_generic_column_scan", |b| {
        b.iter_with_setup(
            || GenericColumnScan::new(&test_column),
            |mut gcs| {
                gcs.seek(randa);
            },
        )
    });
    group.finish();

    let mut group_rle = c.benchmark_group("seek_rle");
    group_rle.bench_function("seek_rle_randomized", |b| {
        b.iter_with_setup(
            || rle_test_column.iter(),
            |mut rcs| {
                rcs.seek(randa);
            },
        )
    });

    let vec_col_handcrafted = VectorColumn::new(
        (1..100000)
            .chain(200000..400000)
            .chain(600000..800000)
            .collect(),
    );
    let rle_col_handcrafted = RleColumn::new(
        (1..100000)
            .chain(200000..400000)
            .chain(600000..800000)
            .collect(),
    );

    group_rle.bench_function("seek_vec_handcrafted", |b| {
        b.iter_with_setup(
            || vec_col_handcrafted.iter(),
            |mut rcs| {
                rcs.seek(black_box(650000));
            },
        )
    });

    group_rle.bench_function("seek_rle_handcrafted", |b| {
        b.iter_with_setup(
            || rle_col_handcrafted.iter(),
            |mut rcs| {
                rcs.seek(black_box(650000));
            },
        )
    });
    group_rle.finish();
}

criterion_group!(benches, benchmark_seek);
criterion_main!(benches);