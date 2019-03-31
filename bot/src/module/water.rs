use crate::{command, config, currency, db, module, stream_info, utils};
use chrono::{DateTime, Utc};
use failure::format_err;
use futures::Future as _;
use parking_lot::RwLock;
use std::{sync::Arc, time};

#[derive(Clone)]
pub struct Reward {
    user: String,
    amount: i32,
}

pub struct Handler {
    db: db::Database,
    currency: currency::Currency,
    cooldown: utils::Cooldown,
    waters: Vec<(DateTime<Utc>, Option<Reward>)>,
    stream_info: Arc<RwLock<stream_info::StreamInfo>>,
}

impl Handler {
    fn check_waters(
        &mut self,
        ctx: &mut command::Context<'_, '_>,
    ) -> Result<(DateTime<Utc>, Option<Reward>), failure::Error> {
        if let Some((when, user)) = self.waters.last() {
            return Ok((when.clone(), user.clone()));
        }

        let started_at = self
            .stream_info
            .read()
            .stream
            .as_ref()
            .map(|s| s.started_at.clone());

        let started_at = match started_at {
            Some(started_at) => started_at,
            None => {
                ctx.respond("Sorry, the !water command is currently not available :(");
                failure::bail!("can't determine start time for stream");
            }
        };

        self.waters.push((started_at.clone(), None));
        Ok((started_at, None))
    }
}

impl command::Handler for Handler {
    fn handle<'m>(&mut self, mut ctx: command::Context<'_, '_>) -> Result<(), failure::Error> {
        if !self.cooldown.is_open() {
            ctx.respond("A !water command was recently issued, please wait a bit longer!");
            return Ok(());
        }

        match ctx.next() {
            Some("undo") => {
                ctx.check_moderator()?;
                let (_, reward) = self.check_waters(&mut ctx)?;

                self.waters.pop();

                let reward = match reward {
                    Some(reward) => reward,
                    None => {
                        ctx.respond("No one has been rewarded for !water yet cmonBruh");
                        return Ok(());
                    }
                };

                ctx.privmsg(format!(
                    "{user} issued a bad !water that is now being undone FeelsBadMan",
                    user = reward.user
                ));

                ctx.spawn(
                    self.db
                        .balance_add(ctx.user.target, reward.user.as_str(), -reward.amount)
                        .map_err(|e| {
                            log::error!("failed to undo water from database: {}", e);
                        }),
                );
            }
            None => {
                let (last, _) = self.check_waters(&mut ctx)?;
                let now = Utc::now();
                let diff = now.clone() - last;
                let amount = i64::max(0i64, diff.num_minutes()) as i32;

                self.waters.push((
                    now,
                    Some(Reward {
                        user: ctx.user.name.to_string(),
                        amount,
                    }),
                ));

                ctx.respond(format!(
                    "{streamer}, DRINK SOME WATER! {user} has been rewarded {amount} {currency} for the reminder.", streamer = ctx.streamer,
                    user = ctx.user.name,
                    amount = amount,
                    currency = self.currency.name
                ));

                ctx.spawn(
                    self.db
                        .balance_add(ctx.user.target, ctx.user.name, amount)
                        .map_err(|e| {
                            log::error!("failed to update water to database: {}", e);
                        }),
                );
            }
            Some(_) => {
                ctx.respond("Expected: !water, or !water undo.");
            }
        }

        Ok(())
    }
}

pub struct Module {
    cooldown: utils::Cooldown,
}

#[derive(Debug, serde::Deserialize)]
pub struct Config {
    #[serde(default = "default_cooldown")]
    cooldown: utils::Cooldown,
}

fn default_cooldown() -> utils::Cooldown {
    utils::Cooldown::from_duration(time::Duration::from_secs(60))
}

impl Module {
    pub fn load(_config: &config::Config, module: &Config) -> Result<Self, failure::Error> {
        Ok(Module {
            cooldown: module.cooldown.clone(),
        })
    }
}

impl super::Module for Module {
    /// Set up command handlers for this module.
    fn hook(
        &self,
        module::HookContext {
            db,
            handlers,
            currency,
            stream_info,
            ..
        }: module::HookContext<'_>,
    ) -> Result<(), failure::Error> {
        let currency = currency
            .ok_or_else(|| format_err!("currency required for !swearjar module"))?
            .clone();

        handlers.insert(
            "water",
            Handler {
                db: db.clone(),
                currency,
                cooldown: self.cooldown.clone(),
                waters: Vec::new(),
                stream_info: stream_info.clone(),
            },
        );

        Ok(())
    }
}