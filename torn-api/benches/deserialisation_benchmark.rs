use criterion::{criterion_group, criterion_main, Criterion};
use torn_api::{faction, user, ThreadSafeApiClient};

pub fn user_benchmark(c: &mut Criterion) {
    dotenv::dotenv().unwrap();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .unwrap();
    let response = rt.block_on(async {
        let key = std::env::var("APIKEY").expect("api key");
        let client = reqwest::Client::default();

        client
            .torn_api(key)
            .user(|b| {
                b.selections(&[
                    user::Selection::Basic,
                    user::Selection::Discord,
                    user::Selection::Profile,
                    user::Selection::PersonalStats,
                ])
            })
            .await
            .unwrap()
    });

    c.bench_function("user deserialize", |b| {
        b.iter(|| {
            response.basic().unwrap();
            response.discord().unwrap();
            response.profile().unwrap();
            response.personal_stats().unwrap();
        })
    });
}

pub fn faction_benchmark(c: &mut Criterion) {
    dotenv::dotenv().unwrap();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .unwrap();
    let response = rt.block_on(async {
        let key = std::env::var("APIKEY").expect("api key");
        let client = reqwest::Client::default();

        client
            .torn_api(key)
            .faction(|b| b.selections(&[faction::Selection::Basic]))
            .await
            .unwrap()
    });

    c.bench_function("user deserialize", |b| {
        b.iter(|| {
            response.basic().unwrap();
        })
    });
}

criterion_group!(benches, user_benchmark, faction_benchmark);
criterion_main!(benches);
