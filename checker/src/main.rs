mod config;
mod injectors;
mod services;
use std::env;

use base::logging::init_logging;
use base::prelude::{anyhow, tokio, tracing};
use injectors::SERVICES;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    init_logging(&SERVICES.conf.log_config).await?;
    let dcl: &'static str = "MainApp";

    let args: Vec<String> = env::args().collect();
    if args.iter().any(|a| a == "single_shot") {
        tracing::info!(dcl = dcl, "Запущена одноразовая проверка срока действия доменов");
        run_check().await?;
        return Ok(());
    }


    let interval_hours = SERVICES.conf.check_interval_hours;
    let mut interval =
        tokio::time::interval(std::time::Duration::from_secs(interval_hours * 3600));
    tracing::info!(dcl = dcl, "Запущен периодический процесс проверки срока действия доменов");

    loop {
        interval.tick().await;
        if let Err(e) = run_check().await {
            tracing::error!(dcl = dcl, %e, "Ошибка периодической проверки");
        }
    }
}

async fn run_check() -> anyhow::Result<()> {
    let mut domain_checker = SERVICES.domain_checker();
    domain_checker.run().await
}
