use crate::config::{NotifierConfig, ServiceConfig, SourceConfig, CONFIG};
use crate::services::{
    domain_checker::DomainCheckerService,
    notifiers::{BaseNotifierTrait, ConsoleNotifierService, TelegramNotifierService},
    sources::{DomainSourceTrait, FileSourceService, SelectelSourceService},
};
use base::prelude::once_cell::sync::Lazy;

pub static SERVICES: Lazy<ServicesInj> = Lazy::new(|| ServicesInj::new(None));

#[derive(Clone)]
pub struct ServicesInj {
    pub conf: &'static ServiceConfig,
    #[allow(dead_code)]
    dcl: &'static str,
}

impl ServicesInj {
    pub fn new(conf: Option<&'static ServiceConfig>) -> Self {
        let conf = conf.unwrap_or(&CONFIG);
        Self { conf, dcl: "ServicesInj" }
    }

    fn source(&self, name: &str) -> Box<dyn DomainSourceTrait> {
        let conf = &self.conf.sources[name];
        match conf {
            SourceConfig::FileConfig { filename } => {
                Box::new(FileSourceService::new(filename))
            }
            SourceConfig::SelectelConfig { account_id, password, project_name, user } => {
                Box::new(SelectelSourceService::new(
                    account_id,
                    password,
                    project_name,
                    user,
                ))
            }
        }
    }

    fn notifier(&self, name: &str) -> Box<dyn BaseNotifierTrait> {
        let conf = &self.conf.notifiers[name];
        match conf {
            NotifierConfig::Console => Box::new(ConsoleNotifierService::new()),
            NotifierConfig::Telegram { bot_token, chat_id, retries } => {
                Box::new(TelegramNotifierService::new(
                    bot_token,
                    chat_id,
                    Some(retries.to_owned()),
                    None,
                ))
            }
        }
    }

    pub fn domain_checker(&self) -> DomainCheckerService {
        let sources =
            self.conf.sources.iter().map(|(name, _)| self.source(name)).collect();

        let notifiers =
            self.conf.notifiers.iter().map(|(name, _)| self.notifier(name)).collect();

        DomainCheckerService::new(
            sources,
            notifiers,
            self.conf.ssl_alarm_days,
            self.conf.alarm_days,
        )
    }
}
