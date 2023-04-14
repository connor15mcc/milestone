use std::env;

use super::referee::{Referee, Score};
use crate::ai::tree::SearchLimit;
use crate::game::player::{Player, AI};
use crate::{ai::heuristics::NUM_HEURISTICS, DATABASE_URL};
use bincode::serialize;
use chrono::Utc;
use lazy_static::lazy_static;
use log::{debug, info, warn};
use rand::Rng;
use rusqlite::{params, Connection, Result};

lazy_static! {
    static ref NUM_BATCHES: usize =
        env::var("NUM_BATCHES").map_or(10, |elt| match elt.parse() {
            Ok(i) => {
                info!("Using NUM_BATCHES environment variable ({})", i);
                i
            }
            _ => 10,
        });
    static ref NUM_AGENTS_RETAINED: usize = env::var("NUM_AGENTS_RETAINED")
        .map_or(10, |elt| match elt.parse() {
          Ok(i) => {
                info!("Using NUM_AGENTS_RETAINED environment variable ({})", i);
                i
          },
          _ => 10
        });
    static ref NUM_CHILDREN_PER_RETAINED_AGENT: usize =
        env::var("NUM_CHILDREN_PER_RETAINED_AGENT")
            .map_or(2, |elt| match elt.parse() {
              Ok(i) => {
                info!("Using NUM_CHILDREN_PER_RETAINED_AGENT environment variable ({})", i);
                i
              }
              _ => 2
            });
    static ref MAX_PERTURB_AMT: f64 =
        env::var("MAX_PERTURB_AMT").map_or(0.1, |elt| match elt.parse() {
          Ok(i) => {
            info!("Using MAX_PERTURB_AMT environment variable ({})", i);
            i
          }
          _ => 0.1
        });
    static ref PERTURB_DECR: f64 =
        env::var("PERTURB_DECR").map_or(0.1, |elt| match elt.parse() {
          Ok(i) => {
            info!("Using PERTURB_DECR environment variable ({})", i);
            i
          }
          _ => 0.99
        });
    pub static ref NUM_AGENTS: usize =
        env::var("NUM_AGENTS").map_or(36, |elt| match elt.parse() {
          Ok(i) => {
            info!("Using NUM_AGENTS environment variable ({})", i);
            i
          }
          _ => 36
        });
    pub static ref NUM_MATCHES: usize =
        env::var("NUM_MATCHES").map_or(108, |elt| match elt.parse() {
          Ok(i) => {
            info!("Using NUM_MATCHES environment variable ({})", i);
            i
          }
          _ => 108
        });
    static ref AGENT_DEPTH: SearchLimit = env::var("AGENT_DEPTH")
        .map_or(SearchLimit::Depth(4), |elt| SearchLimit::Depth(
            match elt.parse() {
              Ok(i) => {
                info!("Using AGENT_DEPTH environment variable ({})", i);
                i
              }
              _ => 4
            }
        ));
}

pub fn run(initial_batch_num: u32, initial_agents: Option<Vec<AI>>) -> AI {
    let mut batch_num = initial_batch_num;
    let agents = match initial_agents {
        Some(i) => i,
        None => {
            let mut randomized_agents = vec![];
            for _ in 0..*NUM_AGENTS {
                randomized_agents.push(random_agent());
            }
            randomized_agents
        }
    };
    let mut prev_batch = Referee::new(agents, batch_num);

    while batch_num as usize <= *NUM_BATCHES {
        prev_batch = run_one_batch(prev_batch);
        // prev_batch.push_batch().unwrap();
        batch_num += 1
    }
    get_best_agents(prev_batch).first().unwrap().to_owned()
}

fn run_one_batch(mut prev: Referee) -> Referee {
    let old_batch_num = prev.batch_num;
    debug!(
        "Running batch #{old_batch_num} with agents: {:#.3?}",
        prev.agents
    );
    prev.play();
    push_batch(&prev)
        .unwrap_or_else(|e| warn!("Could not push to recovery table: {e}"));
    let old_best_agents = get_best_agents(prev);
    info!("Batch #{old_batch_num} completed with best agents: {old_best_agents:#.3?}");
    let new_agents = mutate(old_best_agents, old_batch_num);

    Referee::new(new_agents, old_batch_num + 1)
}

fn checked_div(a: u32, b: u32) -> f32 {
    if b == 0 {
        0.0
    } else {
        a as f32 / b as f32
    }
}

fn get_best_agents(r: Referee) -> Vec<AI> {
    let mut sorted_agents = r.get_agents_with_elos();
    sorted_agents.sort_by_key(|(elo, _ai)| *elo);

    sorted_agents.reverse();

    sorted_agents
        .into_iter()
        .take(*NUM_AGENTS_RETAINED)
        .map(|elt| elt.1)
        .collect::<Vec<AI>>()
}

fn children_from_agent(parent: AI, perturb_amt: f64) -> Vec<AI> {
    let mut children = vec![];
    let mut rng = rand::thread_rng();

    for _ in 0..*NUM_CHILDREN_PER_RETAINED_AGENT {
        let mut child_weights = parent.weights.to_owned();
        for (idx, w) in child_weights.into_iter().enumerate() {
            child_weights[idx] =
                w * rng.gen_range(1.0 - perturb_amt..1.0 + perturb_amt)
        }
        children.push(AI::new(
            String::default(),
            child_weights,
            AGENT_DEPTH.to_owned(),
        ))
    }

    children
}

fn random_agent() -> AI {
    let mut weights = [1.0; NUM_HEURISTICS];
    let mut rng = rand::thread_rng();
    for (idx, _) in weights.into_iter().enumerate() {
        weights[idx] = rng.gen_range(0.0..1.0);
    }
    AI::new(String::default(), weights, AGENT_DEPTH.to_owned())
}

fn mutate(previous_best: Vec<AI>, time: u32) -> Vec<AI> {
    let mut new_gen = vec![];

    let perturb_amt = *MAX_PERTURB_AMT * PERTURB_DECR.powf((time - 1).into());
    info!("Mutating children with {perturb_amt} perturbance");
    for previous_agent in previous_best.into_iter() {
        new_gen.append(&mut children_from_agent(previous_agent, perturb_amt));
    }

    while new_gen.len() < *NUM_AGENTS {
        new_gen.push(random_agent());
    }

    new_gen
}

fn push_batch(prev: &Referee) -> Result<()> {
    let conn = Connection::open(&*DATABASE_URL).unwrap();

    let agents_with_record: Vec<(&AI, &i16)> =
        prev.agents.iter().zip(prev.elos.iter()).collect();
    let serialized_agents_with_record =
        serialize(&(agents_with_record)).unwrap();
    let timestamp = Utc::now().to_string();
    let batch_id = prev.batch_num;

    conn.execute(
        r#"
        INSERT INTO recovery_table (batch_id, agents, timestamp)
        VALUES (?, ?, ?)
        "#,
        params![batch_id, serialized_agents_with_record, timestamp],
    )?;

    // let mut stmt = conn.prepare("SELECT agents FROM recovery_table")?;
    // let agents_iter = stmt.query_map([], |row| {
    //     let bin: Vec<u8> = row.get(0)?;
    //     let agents_with_record: Vec<(AI, Score)> = bincode::deserialize(&bin).unwrap();
    //     Ok(agents_with_record)
    // })?;

    // for agents in agents_iter {
    //     println!("Found list of agents: {:?}", agents.unwrap());
    // }

    Ok(())
}
