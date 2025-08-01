// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use consensus_config::Stake;
use consensus_types::block::{BlockRef, BlockTimestampMs, Round};
use itertools::Itertools;
use parking_lot::RwLock;

use crate::{
    block::{BlockAPI, VerifiedBlock},
    commit::{sort_sub_dag_blocks, Commit, CommittedSubDag, TrustedCommit},
    context::Context,
    dag_state::DagState,
};

/// The `StorageAPI` trait provides an interface for the block store and has been
/// mostly introduced for allowing to inject the test store in `DagBuilder`.
pub(crate) trait BlockStoreAPI {
    fn get_blocks(&self, refs: &[BlockRef]) -> Vec<Option<VerifiedBlock>>;

    fn gc_round(&self) -> Round;

    fn set_committed(&mut self, block_ref: &BlockRef) -> bool;

    fn is_committed(&self, block_ref: &BlockRef) -> bool;
}

impl BlockStoreAPI
    for parking_lot::lock_api::RwLockWriteGuard<'_, parking_lot::RawRwLock, DagState>
{
    fn get_blocks(&self, refs: &[BlockRef]) -> Vec<Option<VerifiedBlock>> {
        DagState::get_blocks(self, refs)
    }

    fn gc_round(&self) -> Round {
        DagState::gc_round(self)
    }

    fn set_committed(&mut self, block_ref: &BlockRef) -> bool {
        DagState::set_committed(self, block_ref)
    }

    fn is_committed(&self, block_ref: &BlockRef) -> bool {
        DagState::is_committed(self, block_ref)
    }
}

/// Expand a committed sequence of leader into a sequence of sub-dags.
#[derive(Clone)]
pub struct Linearizer {
    /// In memory block store representing the dag state
    context: Arc<Context>,
    dag_state: Arc<RwLock<DagState>>,
}

impl Linearizer {
    pub fn new(context: Arc<Context>, dag_state: Arc<RwLock<DagState>>) -> Self {
        Self { context, dag_state }
    }

    /// Collect the sub-dag and the corresponding commit from a specific leader excluding any duplicates or
    /// blocks that have already been committed (within previous sub-dags).
    fn collect_sub_dag_and_commit(
        &mut self,
        leader_block: VerifiedBlock,
    ) -> (CommittedSubDag, TrustedCommit) {
        let _s = self
            .context
            .metrics
            .node_metrics
            .scope_processing_time
            .with_label_values(&["Linearizer::collect_sub_dag_and_commit"])
            .start_timer();

        // Grab latest commit state from dag state
        let mut dag_state = self.dag_state.write();
        let last_commit_index = dag_state.last_commit_index();
        let last_commit_digest = dag_state.last_commit_digest();
        let last_commit_timestamp_ms = dag_state.last_commit_timestamp_ms();

        // Now linearize the sub-dag starting from the leader block
        let to_commit = Self::linearize_sub_dag(leader_block.clone(), &mut dag_state);

        let timestamp_ms = Self::calculate_commit_timestamp(
            &self.context,
            &mut dag_state,
            &leader_block,
            last_commit_timestamp_ms,
        );

        drop(dag_state);

        // Create the Commit.
        let commit = Commit::new(
            last_commit_index + 1,
            last_commit_digest,
            timestamp_ms,
            leader_block.reference(),
            to_commit
                .iter()
                .map(|block| block.reference())
                .collect::<Vec<_>>(),
        );
        let serialized = commit
            .serialize()
            .unwrap_or_else(|e| panic!("Failed to serialize commit: {}", e));
        let commit = TrustedCommit::new_trusted(commit, serialized);

        // Create the corresponding committed sub dag
        let sub_dag = CommittedSubDag::new(
            leader_block.reference(),
            to_commit,
            timestamp_ms,
            commit.reference(),
        );

        (sub_dag, commit)
    }

    /// Calculates the commit's timestamp. The timestamp will be calculated as the median of leader's parents (leader.round - 1)
    /// timestamps by stake. To ensure that commit timestamp monotonicity is respected it is compared against the `last_commit_timestamp_ms`
    /// and the maximum of the two is returned.
    pub(crate) fn calculate_commit_timestamp(
        context: &Context,
        dag_state: &mut impl BlockStoreAPI,
        leader_block: &VerifiedBlock,
        last_commit_timestamp_ms: BlockTimestampMs,
    ) -> BlockTimestampMs {
        let timestamp_ms = {
            // Select leaders' parent blocks.
            let block_refs = leader_block
                .ancestors()
                .iter()
                .filter(|block_ref| block_ref.round == leader_block.round() - 1)
                .cloned()
                .collect::<Vec<_>>();
            // Get the blocks from dag state which should not fail.
            let blocks = dag_state
                .get_blocks(&block_refs)
                .into_iter()
                .map(|block_opt| block_opt.expect("We should have all blocks in dag state."));
            median_timestamp_by_stake(context, blocks).unwrap_or_else(|e| {
                panic!(
                    "Cannot compute median timestamp for leader block {:?} ancestors: {}",
                    leader_block, e
                )
            })
        };

        // Always make sure that commit timestamps are monotonic, so override if necessary.
        timestamp_ms.max(last_commit_timestamp_ms)
    }

    pub(crate) fn linearize_sub_dag(
        leader_block: VerifiedBlock,
        dag_state: &mut impl BlockStoreAPI,
    ) -> Vec<VerifiedBlock> {
        // The GC round here is calculated based on the last committed round of the leader block. The algorithm will attempt to
        // commit blocks up to this GC round. Once this commit has been processed and written to DagState, then gc round will update
        // and on the processing of the next commit we'll have it already updated, so no need to do any gc_round recalculations here.
        // We just use whatever is currently in DagState.
        let gc_round: Round = dag_state.gc_round();
        let leader_block_ref = leader_block.reference();
        let mut buffer = vec![leader_block];
        let mut to_commit = Vec::new();

        // Perform the recursion without stopping at the highest round round that has been committed per authority. Instead it will
        // allow to commit blocks that are lower than the highest committed round for an authority but higher than gc_round.
        assert!(
            dag_state.set_committed(&leader_block_ref),
            "Leader block with reference {:?} attempted to be committed twice",
            leader_block_ref
        );

        while let Some(x) = buffer.pop() {
            to_commit.push(x.clone());

            let ancestors: Vec<VerifiedBlock> = dag_state
                .get_blocks(
                    &x.ancestors()
                        .iter()
                        .copied()
                        .filter(|ancestor| {
                            ancestor.round > gc_round && !dag_state.is_committed(ancestor)
                        })
                        .collect::<Vec<_>>(),
                )
                .into_iter()
                .map(|ancestor_opt| {
                    ancestor_opt.expect("We should have all uncommitted blocks in dag state.")
                })
                .collect();

            for ancestor in ancestors {
                buffer.push(ancestor.clone());
                assert!(
                    dag_state.set_committed(&ancestor.reference()),
                    "Block with reference {:?} attempted to be committed twice",
                    ancestor.reference()
                );
            }
        }

        // The above code should have not yielded any blocks that are <= gc_round, but just to make sure that we'll never
        // commit anything that should be garbage collected we attempt to prune here as well.
        assert!(
            to_commit.iter().all(|block| block.round() > gc_round),
            "No blocks <= {gc_round} should be committed. Leader round {}, blocks {to_commit:?}.",
            leader_block_ref
        );

        // Sort the blocks of the sub-dag blocks
        sort_sub_dag_blocks(&mut to_commit);

        to_commit
    }

    // This function should be called whenever a new commit is observed. This will
    // iterate over the sequence of committed leaders and produce a list of committed
    // sub-dags.
    pub fn handle_commit(&mut self, committed_leaders: Vec<VerifiedBlock>) -> Vec<CommittedSubDag> {
        if committed_leaders.is_empty() {
            return vec![];
        }

        let mut committed_sub_dags = vec![];
        for leader_block in committed_leaders {
            // Collect the sub-dag generated using each of these leaders and the corresponding commit.
            let (sub_dag, commit) = self.collect_sub_dag_and_commit(leader_block);

            self.update_blocks_pruned_metric(&sub_dag);

            // Buffer commit in dag state for persistence later.
            // This also updates the last committed rounds.
            self.dag_state.write().add_commit(commit.clone());

            committed_sub_dags.push(sub_dag);
        }

        committed_sub_dags
    }

    // Try to measure the number of blocks that get pruned due to GC. This is not very accurate, but it can give us a good enough idea.
    // We consider a block as pruned when it is an ancestor of a block that has been committed as part of the provided `sub_dag`, but
    // it has not been committed as part of previous commits. Right now we measure this via checking that highest committed round for the authority
    // as we don't an efficient look up functionality to check if a block has been committed or not.
    fn update_blocks_pruned_metric(&self, sub_dag: &CommittedSubDag) {
        let (last_committed_rounds, gc_round) = {
            let dag_state = self.dag_state.read();
            (dag_state.last_committed_rounds(), dag_state.gc_round())
        };

        for block_ref in sub_dag
            .blocks
            .iter()
            .flat_map(|block| block.ancestors())
            .filter(
                |ancestor_ref| {
                    ancestor_ref.round <= gc_round
                        && last_committed_rounds[ancestor_ref.author] != ancestor_ref.round
                }, // If the last committed round is the same as the pruned block's round, then we know for sure that it has been committed and it doesn't count here
                   // as pruned block.
            )
            .unique()
        {
            let hostname = &self.context.committee.authority(block_ref.author).hostname;

            // If the last committed round from this authority is lower than the pruned ancestor in question, then we know for sure that it has not been committed.
            let label_values = if last_committed_rounds[block_ref.author] < block_ref.round {
                &[hostname, "uncommitted"]
            } else {
                // If last committed round is higher for this authority, then we don't really know it's status, but we know that there is a higher committed block from this authority.
                &[hostname, "higher_committed"]
            };

            self.context
                .metrics
                .node_metrics
                .blocks_pruned_on_commit
                .with_label_values(label_values)
                .inc();
        }
    }
}

/// Computes the median timestamp of the blocks weighted by the stake of their authorities.
/// This function assumes each block comes from a different authority of the same round.
/// Error is returned if no blocks are provided or total stake is less than quorum threshold.
pub(crate) fn median_timestamp_by_stake(
    context: &Context,
    blocks: impl Iterator<Item = VerifiedBlock>,
) -> Result<BlockTimestampMs, String> {
    let mut total_stake = 0;
    let mut timestamps = vec![];
    for block in blocks {
        let stake = context.committee.authority(block.author()).stake;
        timestamps.push((block.timestamp_ms(), stake));
        total_stake += stake;
    }

    if timestamps.is_empty() {
        return Err("No blocks provided".to_string());
    }
    if total_stake < context.committee.quorum_threshold() {
        return Err(format!(
            "Total stake {} < quorum threshold {}",
            total_stake,
            context.committee.quorum_threshold()
        )
        .to_string());
    }

    Ok(median_timestamps_by_stake_inner(timestamps, total_stake))
}

fn median_timestamps_by_stake_inner(
    mut timestamps: Vec<(BlockTimestampMs, Stake)>,
    total_stake: Stake,
) -> BlockTimestampMs {
    timestamps.sort_by_key(|(ts, _)| *ts);

    let mut cumulative_stake = 0;
    for (ts, stake) in &timestamps {
        cumulative_stake += stake;
        if cumulative_stake > total_stake / 2 {
            return *ts;
        }
    }

    timestamps.last().unwrap().0
}

#[cfg(test)]
mod tests {
    use consensus_config::AuthorityIndex;
    use rstest::rstest;

    use super::*;
    use crate::{
        commit::{CommitAPI as _, CommitDigest, DEFAULT_WAVE_LENGTH},
        context::Context,
        leader_schedule::{LeaderSchedule, LeaderSwapTable},
        storage::mem_store::MemStore,
        test_dag_builder::DagBuilder,
        test_dag_parser::parse_dag,
        CommitIndex, TestBlock,
    };

    #[rstest]
    #[tokio::test]
    async fn test_handle_commit() {
        telemetry_subscribers::init_for_testing();
        let num_authorities = 4;
        let (context, _keys) = Context::new_for_test(num_authorities);
        let context = Arc::new(context);

        let dag_state = Arc::new(RwLock::new(DagState::new(
            context.clone(),
            Arc::new(MemStore::new()),
        )));
        let mut linearizer = Linearizer::new(context.clone(), dag_state.clone());

        // Populate fully connected test blocks for round 0 ~ 10, authorities 0 ~ 3.
        let num_rounds: u32 = 10;
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder
            .layers(1..=num_rounds)
            .build()
            .persist_layers(dag_state.clone());

        let leaders = dag_builder
            .leader_blocks(1..=num_rounds)
            .into_iter()
            .map(Option::unwrap)
            .collect::<Vec<_>>();

        let commits = linearizer.handle_commit(leaders.clone());
        for (idx, subdag) in commits.into_iter().enumerate() {
            tracing::info!("{subdag:?}");
            assert_eq!(subdag.leader, leaders[idx].reference());

            let expected_ts = {
                let block_refs = leaders[idx]
                    .ancestors()
                    .iter()
                    .filter(|block_ref| block_ref.round == leaders[idx].round() - 1)
                    .cloned()
                    .collect::<Vec<_>>();
                let blocks = dag_state
                    .read()
                    .get_blocks(&block_refs)
                    .into_iter()
                    .map(|block_opt| block_opt.expect("We should have all blocks in dag state."));

                median_timestamp_by_stake(&context, blocks).unwrap()
            };
            assert_eq!(subdag.timestamp_ms, expected_ts);

            if idx == 0 {
                // First subdag includes the leader block only
                assert_eq!(subdag.blocks.len(), 1);
            } else {
                // Every subdag after will be missing the leader block from the previous
                // committed subdag
                assert_eq!(subdag.blocks.len(), num_authorities);
            }
            for block in subdag.blocks.iter() {
                assert!(block.round() <= leaders[idx].round());
            }
            assert_eq!(subdag.commit_ref.index, idx as CommitIndex + 1);
        }
    }

    #[rstest]
    #[tokio::test]
    async fn test_handle_already_committed() {
        telemetry_subscribers::init_for_testing();
        let num_authorities = 4;
        let (context, _) = Context::new_for_test(num_authorities);
        let context = Arc::new(context);

        let dag_state = Arc::new(RwLock::new(DagState::new(
            context.clone(),
            Arc::new(MemStore::new()),
        )));
        let leader_schedule = Arc::new(LeaderSchedule::new(
            context.clone(),
            LeaderSwapTable::default(),
        ));
        let mut linearizer = Linearizer::new(context.clone(), dag_state.clone());
        let wave_length = DEFAULT_WAVE_LENGTH;

        let leader_round_wave_1 = 3;
        let leader_round_wave_2 = leader_round_wave_1 + wave_length;

        // Build a Dag from round 1..=6
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder.layers(1..=leader_round_wave_2).build();

        // Now retrieve all the blocks up to round leader_round_wave_1 - 1
        // And then only the leader of round leader_round_wave_1
        // Also store those to DagState
        let mut blocks = dag_builder.blocks(0..=leader_round_wave_1 - 1);
        blocks.push(
            dag_builder
                .leader_block(leader_round_wave_1)
                .expect("Leader block should have been found"),
        );
        dag_state.write().accept_blocks(blocks.clone());

        let first_leader = dag_builder
            .leader_block(leader_round_wave_1)
            .expect("Wave 1 leader round block should exist");
        let mut last_commit_index = 1;
        let first_commit_data = TrustedCommit::new_for_test(
            last_commit_index,
            CommitDigest::MIN,
            0,
            first_leader.reference(),
            blocks.iter().map(|block| block.reference()).collect(),
        );
        dag_state.write().add_commit(first_commit_data);

        // Mark the blocks as committed in DagState. This will allow to correctly detect the committed blocks when the new linearizer logic is enabled.
        for block in blocks.iter() {
            dag_state.write().set_committed(&block.reference());
        }

        // Now take all the blocks from round `leader_round_wave_1` up to round `leader_round_wave_2-1`
        let mut blocks = dag_builder.blocks(leader_round_wave_1..=leader_round_wave_2 - 1);
        // Filter out leader block of round `leader_round_wave_1`
        blocks.retain(|block| {
            !(block.round() == leader_round_wave_1
                && block.author() == leader_schedule.elect_leader(leader_round_wave_1, 0))
        });
        // Add the leader block of round `leader_round_wave_2`
        blocks.push(
            dag_builder
                .leader_block(leader_round_wave_2)
                .expect("Leader block should have been found"),
        );
        // Write them in dag state
        dag_state.write().accept_blocks(blocks.clone());

        let mut blocks: Vec<_> = blocks.into_iter().map(|block| block.reference()).collect();

        // Now get the latest leader which is the leader round of wave 2
        let leader = dag_builder
            .leader_block(leader_round_wave_2)
            .expect("Leader block should exist");

        last_commit_index += 1;
        let expected_second_commit = TrustedCommit::new_for_test(
            last_commit_index,
            CommitDigest::MIN,
            0,
            leader.reference(),
            blocks.clone(),
        );

        let commit = linearizer.handle_commit(vec![leader.clone()]);
        assert_eq!(commit.len(), 1);

        let subdag = &commit[0];
        tracing::info!("{subdag:?}");
        assert_eq!(subdag.leader, leader.reference());
        assert_eq!(subdag.commit_ref.index, expected_second_commit.index());

        let expected_ts = median_timestamp_by_stake(
            &context,
            subdag.blocks.iter().filter_map(|block| {
                if block.round() == subdag.leader.round - 1 {
                    Some(block.clone())
                } else {
                    None
                }
            }),
        )
        .unwrap();
        assert_eq!(subdag.timestamp_ms, expected_ts);

        // Using the same sorting as used in CommittedSubDag::sort
        blocks.sort_by(|a, b| a.round.cmp(&b.round).then_with(|| a.author.cmp(&b.author)));
        assert_eq!(
            subdag
                .blocks
                .clone()
                .into_iter()
                .map(|b| b.reference())
                .collect::<Vec<_>>(),
            blocks
        );
        for block in subdag.blocks.iter() {
            assert!(block.round() <= expected_second_commit.leader().round);
        }
    }

    /// This test will run the linearizer with gc_depth = 3 and make
    /// sure that for the exact same DAG the linearizer will commit different blocks according to the rules.
    #[tokio::test]
    async fn test_handle_commit_with_gc_simple() {
        telemetry_subscribers::init_for_testing();

        const GC_DEPTH: u32 = 3;

        let num_authorities = 4;
        let (mut context, _keys) = Context::new_for_test(num_authorities);
        context
            .protocol_config
            .set_consensus_gc_depth_for_testing(GC_DEPTH);

        let context = Arc::new(context);
        let dag_state = Arc::new(RwLock::new(DagState::new(
            context.clone(),
            Arc::new(MemStore::new()),
        )));
        let mut linearizer = Linearizer::new(context.clone(), dag_state.clone());

        // Authorities of index 0->2 will always creates blocks that see each other, but until round 5 they won't see the blocks of authority 3.
        // For authority 3 we create blocks that connect to all the other authorities.
        // On round 5 we finally make the other authorities see the blocks of authority 3.
        // Practically we "simulate" here a long chain created by authority 3 that is visible in round 5, but due to GC blocks of only round >=2 will
        // be committed, when GC is enabled. When GC is disabled all blocks will be committed for rounds >= 1.
        let dag_str = "DAG {
                Round 0 : { 4 },
                Round 1 : { * },
                Round 2 : {
                    A -> [-D1],
                    B -> [-D1],
                    C -> [-D1],
                    D -> [*],
                },
                Round 3 : {
                    A -> [-D2],
                    B -> [-D2],
                    C -> [-D2],
                },
                Round 4 : { 
                    A -> [-D3],
                    B -> [-D3],
                    C -> [-D3],
                    D -> [A3, B3, C3, D2],
                },
                Round 5 : { * },
            }";

        let (_, dag_builder) = parse_dag(dag_str).expect("Invalid dag");
        dag_builder.print();
        dag_builder.persist_all_blocks(dag_state.clone());

        let leaders = dag_builder
            .leader_blocks(1..=6)
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        let commits = linearizer.handle_commit(leaders.clone());
        for (idx, subdag) in commits.into_iter().enumerate() {
            tracing::info!("{subdag:?}");
            assert_eq!(subdag.leader, leaders[idx].reference());

            let expected_ts = {
                let block_refs = leaders[idx]
                    .ancestors()
                    .iter()
                    .filter(|block_ref| block_ref.round == leaders[idx].round() - 1)
                    .cloned()
                    .collect::<Vec<_>>();
                let blocks = dag_state
                    .read()
                    .get_blocks(&block_refs)
                    .into_iter()
                    .map(|block_opt| block_opt.expect("We should have all blocks in dag state."));

                median_timestamp_by_stake(&context, blocks).unwrap()
            };
            assert_eq!(subdag.timestamp_ms, expected_ts);

            if idx == 0 {
                // First subdag includes the leader block only
                assert_eq!(subdag.blocks.len(), 1);
            } else if idx == 1 {
                assert_eq!(subdag.blocks.len(), 3);
            } else if idx == 2 {
                // We commit:
                // * 1 block on round 4, the leader block
                // * 3 blocks on round 3, as no commit happened on round 3 since the leader was missing
                // * 2 blocks on round 2, again as no commit happened on round 3, we commit the "sub dag" of leader of round 3, which will be another 2 blocks
                assert_eq!(subdag.blocks.len(), 6);
            } else {
                // Now it's going to be the first time that a leader will see the blocks of authority 3 and will attempt to commit
                // the long chain. However, due to GC it will only commit blocks of round > 1. That's because it will commit blocks
                // up to previous leader's round (round = 4) minus the gc_depth = 3, so that will be gc_round = 4 - 3 = 1. So we expect
                // to see on the sub dag committed blocks of round >= 2.
                assert_eq!(subdag.blocks.len(), 5);

                assert!(
                    subdag.blocks.iter().all(|block| block.round() >= 2),
                    "Found blocks that are of round < 2."
                );

                // Also ensure that gc_round has advanced with the latest committed leader
                assert_eq!(dag_state.read().gc_round(), subdag.leader.round - GC_DEPTH);
            }
            for block in subdag.blocks.iter() {
                assert!(block.round() <= leaders[idx].round());
            }
            assert_eq!(subdag.commit_ref.index, idx as CommitIndex + 1);
        }
    }

    #[tokio::test]
    async fn test_handle_commit_below_highest_committed_round() {
        telemetry_subscribers::init_for_testing();

        const GC_DEPTH: u32 = 3;

        let num_authorities = 4;
        let (mut context, _keys) = Context::new_for_test(num_authorities);
        context
            .protocol_config
            .set_consensus_gc_depth_for_testing(GC_DEPTH);

        let context = Arc::new(context);
        let dag_state = Arc::new(RwLock::new(DagState::new(
            context.clone(),
            Arc::new(MemStore::new()),
        )));
        let mut linearizer = Linearizer::new(context.clone(), dag_state.clone());

        // Authority D will create an "orphaned" block on round 1 as it won't reference to it on the block of round 2. Similar, no other authority will reference to it on round 2.
        // Then on round 3 the authorities A, B & C will link to block D1. Once the DAG gets committed we should see the block D1 getting committed as well. Normally ,as block D2 would
        // have been committed first block D1 should be ommitted. With the new logic this is no longer true.
        let dag_str = "DAG {
                Round 0 : { 4 },
                Round 1 : { * },
                Round 2 : {
                    A -> [-D1],
                    B -> [-D1],
                    C -> [-D1],
                    D -> [-D1],
                },
                Round 3 : {
                    A -> [A2, B2, C2, D1],
                    B -> [A2, B2, C2, D1],
                    C -> [A2, B2, C2, D1],
                    D -> [A2, B2, C2, D2]
                },
                Round 4 : { * },
            }";

        let (_, dag_builder) = parse_dag(dag_str).expect("Invalid dag");
        dag_builder.print();
        dag_builder.persist_all_blocks(dag_state.clone());

        let leaders = dag_builder
            .leader_blocks(1..=4)
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        let commits = linearizer.handle_commit(leaders.clone());
        for (idx, subdag) in commits.into_iter().enumerate() {
            tracing::info!("{subdag:?}");
            assert_eq!(subdag.leader, leaders[idx].reference());

            let expected_ts = {
                let block_refs = leaders[idx]
                    .ancestors()
                    .iter()
                    .filter(|block_ref| block_ref.round == leaders[idx].round() - 1)
                    .cloned()
                    .collect::<Vec<_>>();
                let blocks = dag_state
                    .read()
                    .get_blocks(&block_refs)
                    .into_iter()
                    .map(|block_opt| block_opt.expect("We should have all blocks in dag state."));

                median_timestamp_by_stake(&context, blocks).unwrap()
            };
            assert_eq!(subdag.timestamp_ms, expected_ts);

            if idx == 0 {
                // First subdag includes the leader block only B1
                assert_eq!(subdag.blocks.len(), 1);
            } else if idx == 1 {
                // We commit:
                // * 1 block on round 2, the leader block C2
                // * 2 blocks on round 1, A1, C1
                assert_eq!(subdag.blocks.len(), 3);
            } else if idx == 2 {
                // We commit:
                // * 1 block on round 3, the leader block D3
                // * 3 blocks on round 2, A2, B2, D2
                assert_eq!(subdag.blocks.len(), 4);

                assert!(
                    subdag.blocks.iter().any(|block| block.round() == 2
                        && block.author() == AuthorityIndex::new_for_test(3)),
                    "Block D2 should have been committed."
                );
            } else if idx == 3 {
                // We commit:
                // * 1 block on round 4, the leader block A4
                // * 3 blocks on round 3, A3, B3, C3
                // * 1 block of round 1, D1
                assert_eq!(subdag.blocks.len(), 5);
                assert!(
                    subdag.blocks.iter().any(|block| block.round() == 1
                        && block.author() == AuthorityIndex::new_for_test(3)),
                    "Block D1 should have been committed."
                );
            } else {
                panic!("Unexpected subdag with index {:?}", idx);
            }

            for block in subdag.blocks.iter() {
                assert!(block.round() <= leaders[idx].round());
            }
            assert_eq!(subdag.commit_ref.index, idx as CommitIndex + 1);
        }
    }

    #[rstest]
    #[case(3_000, 3_000, 6_000)]
    #[tokio::test]
    async fn test_calculate_commit_timestamp(
        #[case] timestamp_1: u64,
        #[case] timestamp_2: u64,
        #[case] timestamp_3: u64,
    ) {
        // GIVEN
        telemetry_subscribers::init_for_testing();

        let num_authorities = 4;
        let (context, _keys) = Context::new_for_test(num_authorities);

        let context = Arc::new(context);
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));
        let mut dag_state = dag_state.write();

        let ancestors = vec![
            VerifiedBlock::new_for_test(TestBlock::new(4, 0).set_timestamp_ms(1_000).build()),
            VerifiedBlock::new_for_test(TestBlock::new(4, 1).set_timestamp_ms(2_000).build()),
            VerifiedBlock::new_for_test(TestBlock::new(4, 2).set_timestamp_ms(3_000).build()),
            VerifiedBlock::new_for_test(TestBlock::new(4, 3).set_timestamp_ms(4_000).build()),
        ];

        let leader_block = VerifiedBlock::new_for_test(
            TestBlock::new(5, 0)
                .set_timestamp_ms(5_000)
                .set_ancestors(
                    ancestors
                        .iter()
                        .map(|block| block.reference())
                        .collect::<Vec<_>>(),
                )
                .build(),
        );

        for block in &ancestors {
            dag_state.accept_block(block.clone());
        }

        let last_commit_timestamp_ms = 0;

        // WHEN
        let timestamp = Linearizer::calculate_commit_timestamp(
            &context,
            &mut dag_state,
            &leader_block,
            last_commit_timestamp_ms,
        );
        assert_eq!(timestamp, timestamp_1);

        // AND skip the block of authority 0 and round 4.
        let leader_block = VerifiedBlock::new_for_test(
            TestBlock::new(5, 0)
                .set_timestamp_ms(5_000)
                .set_ancestors(
                    ancestors
                        .iter()
                        .skip(1)
                        .map(|block| block.reference())
                        .collect::<Vec<_>>(),
                )
                .build(),
        );

        let timestamp = Linearizer::calculate_commit_timestamp(
            &context,
            &mut dag_state,
            &leader_block,
            last_commit_timestamp_ms,
        );
        assert_eq!(timestamp, timestamp_2);

        // AND set the `last_commit_timestamp_ms` to 6_000
        let last_commit_timestamp_ms = 6_000;
        let timestamp = Linearizer::calculate_commit_timestamp(
            &context,
            &mut dag_state,
            &leader_block,
            last_commit_timestamp_ms,
        );
        assert_eq!(timestamp, timestamp_3);

        // AND there is only one ancestor block to commit
        let (context, _) = Context::new_for_test(1);
        let leader_block = VerifiedBlock::new_for_test(
            TestBlock::new(5, 0)
                .set_timestamp_ms(5_000)
                .set_ancestors(
                    ancestors
                        .iter()
                        .take(1)
                        .map(|block| block.reference())
                        .collect::<Vec<_>>(),
                )
                .build(),
        );
        let last_commit_timestamp_ms = 0;
        let timestamp = Linearizer::calculate_commit_timestamp(
            &context,
            &mut dag_state,
            &leader_block,
            last_commit_timestamp_ms,
        );
        assert_eq!(timestamp, 1_000);
    }

    #[test]
    fn test_median_timestamps_by_stake() {
        // One total stake.
        let timestamps = vec![(1_000, 1)];
        assert_eq!(median_timestamps_by_stake_inner(timestamps, 1), 1_000);

        // Odd number of total stakes.
        let timestamps = vec![(1_000, 1), (2_000, 1), (3_000, 1)];
        assert_eq!(median_timestamps_by_stake_inner(timestamps, 3), 2_000);

        // Even number of total stakes.
        let timestamps = vec![(1_000, 1), (2_000, 1), (3_000, 1), (4_000, 1)];
        assert_eq!(median_timestamps_by_stake_inner(timestamps, 4), 3_000);

        // Even number of total stakes, different order.
        let timestamps = vec![(4_000, 1), (3_000, 1), (1_000, 1), (2_000, 1)];
        assert_eq!(median_timestamps_by_stake_inner(timestamps, 4), 3_000);

        // Unequal stakes.
        let timestamps = vec![(2_000, 2), (4_000, 2), (1_000, 3), (3_000, 3)];
        assert_eq!(median_timestamps_by_stake_inner(timestamps, 10), 3_000);

        // Unequal stakes.
        let timestamps = vec![
            (500, 2),
            (4_000, 2),
            (2_500, 3),
            (1_000, 5),
            (3_000, 3),
            (2_000, 4),
        ];
        assert_eq!(median_timestamps_by_stake_inner(timestamps, 19), 2_000);

        // One authority dominates.
        let timestamps = vec![(1_000, 1), (2_000, 1), (3_000, 1), (4_000, 1), (5_000, 10)];
        assert_eq!(median_timestamps_by_stake_inner(timestamps, 14), 5_000);
    }

    #[tokio::test]
    async fn test_median_timestamps_by_stake_errors() {
        let num_authorities = 4;
        let (context, _keys) = Context::new_for_test(num_authorities);
        let context = Arc::new(context);

        // No blocks provided
        let err = median_timestamp_by_stake(&context, vec![].into_iter()).unwrap_err();
        assert_eq!(err, "No blocks provided");

        // Blocks provided but total stake is less than quorum threshold
        let block = VerifiedBlock::new_for_test(TestBlock::new(5, 0).build());
        let err = median_timestamp_by_stake(&context, vec![block].into_iter()).unwrap_err();
        assert_eq!(err, "Total stake 1 < quorum threshold 3");
    }
}
