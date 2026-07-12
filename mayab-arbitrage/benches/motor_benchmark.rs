use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mayab_arbitrage::config::Config;
use mayab_arbitrage::motor::Motor;
use mayab_arbitrage::types::{Cotizacion, NivelOrden};
use std::sync::Arc;
use tokio::runtime::Runtime;

fn cotizacion(exchange: &str, bid: f64, ask: f64, bid_qty: f64, ask_qty: f64) -> Cotizacion {
    Cotizacion {
        exchange: exchange.to_string(),
        par: "BTC/USDT".to_string(),
        bid,
        bid_cantidad: bid_qty,
        ask,
        ask_cantidad: ask_qty,
        bids: vec![NivelOrden {
            precio: bid,
            cantidad: bid_qty,
        }]
        .into(),
        asks: vec![NivelOrden {
            precio: ask,
            cantidad: ask_qty,
        }]
        .into(),
        evento_unix_ms: 0,
        recibida_en: chrono::Utc::now(),
        latencia_ms: 0,
        secuencia: 0,
        exchange_sequence: None,
        integrity_status: "test_snapshot".to_string(),
        resyncs: 0,
        timestamp_confiable: true,
        conectado: true,
        ultimo_mensaje: String::new(),
    }
}

fn motor_recibir_cotizacion_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let costos = Config::from_env().costos;
    let motor = Arc::new(Motor::new(
        costos,
        10000.0,
        1.0,
        "BTC-USD".to_string(),
        Vec::<String>::new(),
        None,
    ));

    let cotizacion_compra = cotizacion("Binance", 50000.0, 50010.0, 2.0, 2.0);
    let cotizacion_venta = cotizacion("Kraken", 50200.0, 50210.0, 1.5, 1.5);

    rt.block_on(async {
        motor.recibir_cotizacion(cotizacion_compra.clone()).await;
        motor.recibir_cotizacion(cotizacion_venta.clone()).await;
    });

    c.bench_function("motor_recibir_cotizacion", |b| {
        b.iter(|| {
            rt.block_on(async {
                let estado = motor.estado().await;
                black_box(estado.cotizaciones.len());
                motor.recibir_cotizacion(cotizacion_venta.clone()).await;
            });
        });
    });
}

criterion_group!(benches, motor_recibir_cotizacion_benchmark);
criterion_main!(benches);
