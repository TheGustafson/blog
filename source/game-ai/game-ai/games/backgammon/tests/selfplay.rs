use ai_backgammon::SearchOptions;
use ai_backgammon::selfplay::{AgentConfig, ArenaConfig, run_paired};

fn search(depth: u8, nodes: u64) -> AgentConfig {
    AgentConfig::Expectimax(SearchOptions {
        max_depth: depth,
        node_limit: nodes,
        soft_time_ms: 0,
    })
}

#[test]
fn paired_arena_swaps_colors_and_uses_two_games_per_pair() {
    let report = run_paired(
        AgentConfig::FirstLegal,
        AgentConfig::FirstLegal,
        ArenaConfig {
            pairs: 2,
            seed: 0x91e4_723a_410b_c933,
            max_turns: 1_000,
        },
    )
    .unwrap();
    assert_eq!(report.games, 4);
    assert_eq!(report.a_as_white, 2);
    assert_eq!(report.a_as_black, 2);
    assert_eq!(report.a_wins + report.b_wins, report.games);
    assert_eq!(
        report.singles + report.gammons + report.backgammons,
        report.games
    );
}

#[test]
fn fixed_seed_self_play_is_reproducible() {
    let config = ArenaConfig {
        pairs: 2,
        seed: 0xc638_41ad_d19e_2a77,
        max_turns: 1_000,
    };
    let first = run_paired(AgentConfig::Static, AgentConfig::Pip, config).unwrap();
    let second = run_paired(AgentConfig::Static, AgentConfig::Pip, config).unwrap();
    assert_eq!(first, second);
}

#[test]
fn points_match_the_recorded_game_kinds() {
    let report = run_paired(
        AgentConfig::Static,
        AgentConfig::FirstLegal,
        ArenaConfig {
            pairs: 2,
            seed: 0x788e_5f90_162b_70da,
            max_turns: 1_000,
        },
    )
    .unwrap();
    assert!(report.a_points + report.b_points >= report.games);
    assert!(report.a_points + report.b_points <= report.games * 3);
}

#[test]
fn every_baseline_can_finish_paired_games() {
    let config = ArenaConfig {
        pairs: 1,
        seed: 0xa347_129f_c064_f265,
        max_turns: 1_000,
    };
    for agent in [
        AgentConfig::FirstLegal,
        AgentConfig::Pip,
        AgentConfig::Static,
        search(1, 20_000),
    ] {
        let report = run_paired(agent, AgentConfig::FirstLegal, config).unwrap();
        assert_eq!(report.games, 2);
        if matches!(agent, AgentConfig::Expectimax(_)) {
            assert!(report.a_searches > 0);
            assert!(report.a_depth > 0);
        }
    }
}

#[test]
fn search_clears_the_first_release_gate_against_first_legal() {
    let report = run_paired(
        search(1, 50_000),
        AgentConfig::FirstLegal,
        ArenaConfig {
            pairs: 8,
            seed: 0x104d_9e62_30f7_a8c1,
            max_turns: 1_000,
        },
    )
    .unwrap();
    assert!(
        report.a_points > report.b_points,
        "expectimax did not clear its first gate: {report:?}",
    );
}

#[test]
fn deeper_search_is_screened_against_the_static_evaluator() {
    let report = run_paired(
        search(2, 100_000),
        AgentConfig::Static,
        ArenaConfig {
            pairs: 4,
            seed: 0xbee3_b4fb_9127_194a,
            max_turns: 1_000,
        },
    )
    .unwrap();
    assert!(
        report.a_points >= report.b_points,
        "deeper search regressed against static play: {report:?}",
    );
}
