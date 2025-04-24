include!(concat!(env!("OUT_DIR"), "/scopes.rs"));

#[cfg(test)]
pub(super) mod test {
    use std::{collections::VecDeque, sync::OnceLock, time::Duration};

    use tokio::sync::mpsc;

    use crate::{
        executor::{ExecutorExt, ReqwestClient},
        models::{
            AttackCode, FactionSelectionName, PersonalStatsCategoryEnum, PersonalStatsStatName,
            UserListEnum,
        },
    };

    use super::*;

    static TICKETS: OnceLock<mpsc::Sender<mpsc::Sender<ReqwestClient>>> = OnceLock::new();

    pub(crate) async fn test_client() -> ReqwestClient {
        let (tx, mut rx) = mpsc::channel(1);

        let ticket_tx = TICKETS
            .get_or_init(|| {
                let (tx, mut rx) = mpsc::channel(1);
                std::thread::spawn(move || {
                    let mut queue = VecDeque::<mpsc::Sender<ReqwestClient>>::new();

                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_time()
                        .build()
                        .unwrap();

                    rt.block_on(async move {
                        loop {
                            tokio::select! {
                                recv_result = rx.recv() => {
                                    match recv_result {
                                        Some(ticket) => queue.push_back(ticket),
                                        None => break,
                                    }
                                }
                                _ = tokio::time::sleep(Duration::from_secs(1)) => {
                                    if let Some(next) = queue.pop_front() {
                                        next.send(ReqwestClient::new(&std::env::var("API_KEY").unwrap())).await.unwrap()
                                    }
                                }
                            }
                        }
                    });
                });

                tx
            })
            .clone();

        ticket_tx.send(tx).await.unwrap();

        rx.recv().await.unwrap()
    }

    #[tokio::test]
    async fn faction() {
        let client = test_client().await;

        let r = client
            .faction()
            .for_selections(|b| {
                b.selections([FactionSelectionName::Basic, FactionSelectionName::Balance])
            })
            .await
            .unwrap();

        r.faction_basic_response().unwrap();
    }

    #[tokio::test]
    async fn faction_applications() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope.applications(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn faction_attacks() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope.attacks(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn faction_attacksfull() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope.attacksfull(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn faction_balance() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope.balance(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn faction_basic() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope.basic(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn faction_basic_for_id() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope.basic_for_id(19.into(), |b| b).await.unwrap();
    }

    #[tokio::test]
    async fn faction_chain() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope.chain(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn faction_chain_for_id() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope.chain_for_id(19.into(), |b| b).await.unwrap();
    }

    #[tokio::test]
    async fn faction_chains() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope.chains(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn factions_chains_for_id() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope.chains_for_id(19.into(), |b| b).await.unwrap();
    }

    #[tokio::test]
    async fn faction_chain_report() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope.chainreport(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn faction_chain_report_for_id() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope
            .chainreport_for_chain_id(47004769.into(), |b| b)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn faction_contributors() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope
            .contributors(|b| b.stat(crate::models::FactionStatEnum::Revives))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn faction_crimes() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope.crimes(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn faction_crime_for_id() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope
            .crime_for_crime_id(468347.into(), |b| b)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn faction_hof() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope.hof(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn faction_hof_for_id() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope.hof_for_id(19.into(), |b| b).await.unwrap();
    }

    #[tokio::test]
    async fn faction_members() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope.members(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn faction_members_for_id() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope
            .members_for_id(19.into(), |b| b)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn faction_news() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope
            .news(|b| b.cat(crate::models::FactionNewsCategory::Attack))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn faction_ranked_wars() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope.rankedwars(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn faction_ranked_war_for_id() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope
            .rankedwars_for_id(19.into(), |b| b)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn faction_ranked_war_report_for_id() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope
            .rankedwarreport_for_ranked_war_id(24424.into(), |b| b)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn faction_revives() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope.revives(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn faction_revives_full() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope.revives_full(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn faction_stats() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope.stats(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn faction_upgrades() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope.upgrades(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn faction_wars() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope.wars(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn faction_wars_for_id() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope.wars_for_id(19.into(), |b| b).await.unwrap();
    }

    #[tokio::test]
    async fn faction_lookup() {
        let client = test_client().await;

        let faction_scope = FactionScope(&client);

        faction_scope.lookup(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn forum_categories() {
        let client = test_client().await;

        let forum_scope = ForumScope(&client);

        forum_scope.categories(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn forum_posts_for_thread_id() {
        let client = test_client().await;

        let forum_scope = ForumScope(&client);

        forum_scope
            .posts_for_thread_id(16129703.into(), |b| b)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn forum_thread_for_thread_id() {
        let client = test_client().await;

        let forum_scope = ForumScope(&client);

        forum_scope
            .thread_for_thread_id(16129703.into(), |b| b)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn forum_threads() {
        let client = test_client().await;

        let forum_scope = ForumScope(&client);

        forum_scope.threads(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn forum_threads_for_category_ids() {
        let client = test_client().await;

        let forum_scope = ForumScope(&client);

        forum_scope
            .threads_for_category_ids("2".to_owned(), |b| b)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn forum_lookup() {
        let client = test_client().await;

        let forum_scope = ForumScope(&client);

        forum_scope.lookup(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn forum_timestamp() {
        let client = test_client().await;

        let forum_scope = ForumScope(&client);

        forum_scope.timestamp(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn market_itemmarket_for_id() {
        let client = test_client().await;

        let market_scope = MarketScope(&client);

        market_scope
            .itemmarket_for_id(1.into(), |b| b)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn market_lookup() {
        let client = test_client().await;

        let market_scope = MarketScope(&client);

        market_scope.lookup(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn market_timestamp() {
        let client = test_client().await;

        let market_scope = MarketScope(&client);

        market_scope.timestamp(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn racing_cars() {
        let client = test_client().await;

        let racing_scope = RacingScope(&client);

        racing_scope.cars(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn racing_carupgrades() {
        let client = test_client().await;

        let racing_scope = RacingScope(&client);

        racing_scope.carupgrades(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn racing_races() {
        let client = test_client().await;

        let racing_scope = RacingScope(&client);

        racing_scope.races(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn racing_race_for_race_id() {
        let client = test_client().await;

        let racing_scope = RacingScope(&client);

        racing_scope
            .race_for_race_id(14650821.into(), |b| b)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn racing_tracks() {
        let client = test_client().await;

        let racing_scope = RacingScope(&client);

        racing_scope.tracks(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn racing_lookup() {
        let client = test_client().await;

        let racing_scope = RacingScope(&client);

        racing_scope.lookup(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn racing_timestamp() {
        let client = test_client().await;

        let racing_scope = RacingScope(&client);

        racing_scope.timestamp(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn torn_attacklog() {
        let client = test_client().await;

        let racing_scope = TornScope(&client);

        racing_scope
            .attacklog(|b| b.log(AttackCode("ec987a60a22155cbfb7c1625cbb2092f".to_owned())))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn torn_bounties() {
        let client = test_client().await;

        let torn_scope = TornScope(&client);

        torn_scope.bounties(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn torn_calendar() {
        let client = test_client().await;

        let torn_scope = TornScope(&client);

        torn_scope.calendar(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn torn_crimes() {
        let client = test_client().await;

        let torn_scope = TornScope(&client);

        torn_scope.crimes(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn torn_factionhof() {
        let client = test_client().await;

        let torn_scope = TornScope(&client);

        torn_scope
            .factionhof(|b| b.cat(crate::models::TornFactionHofCategory::Rank))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn torn_factiontree() {
        let client = test_client().await;

        let torn_scope = TornScope(&client);

        torn_scope.factiontree(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn torn_hof() {
        let client = test_client().await;

        let torn_scope = TornScope(&client);

        torn_scope
            .hof(|b| b.cat(crate::models::TornHofCategory::Offences))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn torn_itemammo() {
        let client = test_client().await;

        let torn_scope = TornScope(&client);

        torn_scope.itemammo(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn torn_itemmods() {
        let client = test_client().await;

        let torn_scope = TornScope(&client);

        torn_scope.itemmods(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn torn_items() {
        let client = test_client().await;

        let torn_scope = TornScope(&client);

        torn_scope.items(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn torn_items_for_ids() {
        let client = test_client().await;

        let torn_scope = TornScope(&client);

        torn_scope
            .items_for_ids("1".to_owned(), |b| b)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn torn_logcategories() {
        let client = test_client().await;

        let torn_scope = TornScope(&client);

        torn_scope.logcategories(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn torn_logtypes() {
        let client = test_client().await;

        let torn_scope = TornScope(&client);

        torn_scope.logtypes(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn torn_logtypes_for_log_category_id() {
        let client = test_client().await;

        let torn_scope = TornScope(&client);

        torn_scope
            .logtypes_for_log_category_id(23.into(), |b| b)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn torn_subrcimes_for_crime_id() {
        let client = test_client().await;

        let torn_scope = TornScope(&client);

        torn_scope
            .subcrimes_for_crime_id(3.into(), |b| b)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn torn_lookup() {
        let client = test_client().await;
        let torn_scope = TornScope(&client);

        torn_scope.lookup(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn torn_timestamp() {
        let client = test_client().await;

        let torn_scope = TornScope(&client);

        torn_scope.timestamp(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn user_attacks() {
        let client = test_client().await;

        client.user().attacks(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn user_attacksfull() {
        let client = test_client().await;

        client.user().attacksfull(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn user_bounties() {
        let client = test_client().await;

        client.user().bounties(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn user_bounties_for_id() {
        let client = test_client().await;

        client
            .user()
            .bounties_for_id(986228.into(), |b| b)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn user_calendar() {
        let client = test_client().await;

        client.user().calendar(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn user_crimes_for_crime_id() {
        let client = test_client().await;

        client
            .user()
            .crimes_for_crime_id(10.into(), |b| b)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn user_enlisted_cars() {
        let client = test_client().await;

        client.user().enlistedcars(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn user_factionbalance() {
        let client = test_client().await;

        client.user().factionbalance(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn user_forumfeed() {
        let client = test_client().await;

        client.user().forumfeed(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn user_forumfriends() {
        let client = test_client().await;

        client.user().forumfriends(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn user_forumposts() {
        let client = test_client().await;

        client.user().forumposts(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn user_forumposts_for_id() {
        let client = test_client().await;

        client
            .user()
            .forumposts_for_id(1.into(), |b| b)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn user_forumsubscribedthreads() {
        let client = test_client().await;

        client.user().forumsubscribedthreads(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn user_forumthreads() {
        let client = test_client().await;

        client.user().forumthreads(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn user_forumthreads_for_id() {
        let client = test_client().await;

        client
            .user()
            .forumthreads_for_id(1.into(), |b| b)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn user_hof() {
        let client = test_client().await;

        client.user().hof(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn user_hof_for_id() {
        let client = test_client().await;

        client.user().hof_for_id(1.into(), |b| b).await.unwrap();
    }

    #[tokio::test]
    async fn user_itemmarket() {
        let client = test_client().await;

        client.user().itemmarket(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn user_jobranks() {
        let client = test_client().await;

        client.user().jobranks(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn user_list() {
        let client = test_client().await;

        client
            .user()
            .list(|b| b.cat(UserListEnum::Friends))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn user_organizedcrime() {
        let client = test_client().await;

        client.user().organizedcrime(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn user_personalstats() {
        let client = test_client().await;

        client
            .user()
            .personalstats(|b| {
                b.stat([PersonalStatsStatName::Piercinghits])
                    .timestamp(1737661955)
            })
            .await
            .unwrap();

        client
            .user()
            .personalstats(|b| b.cat(PersonalStatsCategoryEnum::All))
            .await
            .unwrap();

        client
            .user()
            .personalstats(|b| b.cat(PersonalStatsCategoryEnum::Popular))
            .await
            .unwrap();

        client
            .user()
            .personalstats(|b| b.cat(PersonalStatsCategoryEnum::Drugs))
            .await
            .unwrap();

        client
            .user()
            .personalstats(|b| b.stat([PersonalStatsStatName::Piercinghits]))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn user_personalstats_for_id() {
        let client = test_client().await;

        client
            .user()
            .personalstats_for_id(1.into(), |b| b.cat(PersonalStatsCategoryEnum::All))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn user_races() {
        let client = test_client().await;

        client.user().races(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn user_revives() {
        let client = test_client().await;

        client.user().revives(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn user_revivesfull() {
        let client = test_client().await;

        client.user().revives_full(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn user_lookup() {
        let client = test_client().await;

        client.user().lookup(|b| b).await.unwrap();
    }

    #[tokio::test]
    async fn user_timestamp() {
        let client = test_client().await;

        client.user().attacks(|b| b).await.unwrap();
    }
}
