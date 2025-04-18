use anyhow::Result;
use plonky2::{
    iop::{target::BoolTarget, witness::PartialWitness},
    plonk::circuit_builder::CircuitBuilder,
};

use crate::pod::circuit::pod::SchnorrPODTarget;
use crate::{
    pod::POD,
    recursion::{utils::assert_one_if_enabled, InnerCircuitTrait},
    D, F,
};

/// NS stands for NumStatements, the number of statements checked in the POD.
pub struct SchnorrPODGadget<const NS: usize>;

impl<const NS: usize> InnerCircuitTrait for SchnorrPODGadget<NS> {
    type Input = POD;
    type Targets = SchnorrPODTarget;

    /// set up the circuit logic
    fn add_targets(
        builder: &mut CircuitBuilder<F, D>,
        selector_booltarg: &BoolTarget,
    ) -> Result<Self::Targets> {
        let schnorr_pod_target = SchnorrPODTarget::new_virtual(builder, NS);

        // Check thaht payload is valid.
        schnorr_pod_target.check_payload(builder);

        // Compute hash target.
        let hash_target = schnorr_pod_target.compute_hash_target(builder);

        // Add POD in-circuit verification logic.
        let (_, verified) = schnorr_pod_target.compute_targets_and_verify(builder, &hash_target)?;

        // If selector_booltarg=1, check verified.target.
        assert_one_if_enabled(builder, verified.target, selector_booltarg);
        Ok(schnorr_pod_target)
    }

    /// set the actual witness values for the current instance of the circuit
    fn set_targets(
        pw: &mut PartialWitness<F>,
        pod_target: &Self::Targets, // targets = schnorr_pod_target
        pod: &Self::Input,          // input = pod
    ) -> Result<Vec<F>> {
        pod_target.set_witness(pw, pod)?;
        // no public inputs at SchnorrPODGadget, return empty vec
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use plonky2::{
        field::{goldilocks_field::GoldilocksField, types::Field},
        iop::witness::{PartialWitness, WitnessWrite},
        plonk::{circuit_builder::CircuitBuilder, circuit_data::CircuitConfig},
    };

    use super::*;
    use crate::{
        pod::{entry::Entry, payload::HashablePayload, POD},
        signature::schnorr::SchnorrSecretKey,
    };
    use crate::{C, D, F};

    #[test]
    fn schnorr_pod_test() -> Result<()> {
        const NS: usize = 2; // NS: NumStatements
        const VL: usize = 0; // VL: Vector Length

        let scalar1 = GoldilocksField(36);
        let entry1 = Entry::new_from_scalar("some key", scalar1);
        let schnorr_pod3 =
            POD::execute_schnorr_gadget::<NS, VL>(&[entry1.clone()], &SchnorrSecretKey { sk: 25 })?;
        let payload_hash = schnorr_pod3.payload.hash_payload();

        let config = CircuitConfig::standard_recursion_config();
        let mut builder = CircuitBuilder::<F, D>::new(config);

        let selector_targ = builder.add_virtual_target();
        let selector_booltarg = BoolTarget::new_unsafe(selector_targ);

        let hash_target = builder.add_virtual_hash_public_input();

        let schnorr_pod_target =
            SchnorrPODGadget::<NS>::add_targets(&mut builder, &selector_booltarg)?;

        // set selector=1, so that the pod is verified in the InnerCircuit
        let selector = F::ONE;

        // Assign witnesses
        let mut pw: PartialWitness<F> = PartialWitness::new();
        pw.set_target(selector_targ, selector)?;
        pw.set_hash_target(hash_target, payload_hash)?;
        SchnorrPODGadget::<NS>::set_targets(&mut pw, &schnorr_pod_target, &schnorr_pod3)?;

        // Build and prove.
        let data = builder.build::<C>();
        let proof = data.prove(pw)?;
        data.verify(proof.clone())?;
        Ok(())
    }

    /*
        use crate::pod::circuit::operation::OpExecutorGadget;
        use crate::recursion::{traits_examples::ExampleOpsExecutor, RecursionTree};
        use crate::PlonkyProof;
        use hashbrown::HashMap;
        use plonky2::plonk::proof::ProofWithPublicInputs;
        use plonky2::recursion::dummy_circuit::cyclic_base_proof;
        use std::array;
        use std::time::Instant;

        #[test]
        fn test_recursion_framework_with_SchnorrPODGadget() -> Result<()> {
            const NS: usize = 2; // NS: NumStatements
            const M: usize = 2; // number of InnerCircuits at each recursive node
            const N: usize = 2; // arity of the recursion tree
            let l: usize = 2; // levels of the recursion (binary) tree

            // build the pods, one for each leaf of the recursion tree
            let pods: Vec<POD> = (0..M + N)
                .into_iter()
                .map(|i| {
                    let scalar1 = GoldilocksField(36 + (i as u64));
                    let entry1 = Entry::new_from_scalar("some key", scalar1);
                    POD::execute_schnorr_gadget(&vec![entry1.clone()], &SchnorrSecretKey { sk: 25 })
                })
                .collect();

            type RT = RecursionTree<SchnorrPODGadget<NS>, OpExecutorGadget<'static, 2, 3>, M, N>;

            // build the circuit_data & verifier_data for the recursive circuit
            let circuit_data = RT::circuit_data()?;
            let verifier_data = circuit_data.verifier_data();

            let dummy_proof_pis = cyclic_base_proof(
                &circuit_data.common,
                &verifier_data.verifier_only,
                HashMap::new(),
            );
            let dummy_proof = dummy_proof_pis.proof;

            // we start with k dummy proofs, since at the leafs level we don't have proofs yet and we
            // just verify the signatures. At each level we divide the amount of proofs by N. At the
            // root level there is a single proof.
            let mut proofs_at_level_i: Vec<PlonkyProof> = (0..(N * N))
                .into_iter()
                .map(|_| dummy_proof.clone())
                .collect();

            // loop over the recursion levels
            for i in 0..l {
                println!("\n--- recursion level i={}", i);
                let mut next_level_proofs: Vec<PlonkyProof> = vec![];

                // loop over the nodes of each recursion tree level
                for j in (0..proofs_at_level_i.len()).into_iter().step_by(N) {
                    println!(
                        "\n------ recursion node: (level) i={}, (node in level) j={}",
                        i, j
                    );

                    // base level: enable InnerCircuit, rest: enable recursive proof verification
                    let mut selectors: [F; M + N] = [F::ZERO; M + N];
                    if i > 0 {
                        // if we're not at the base-level, enable the N selectors of the proofs
                        // verifications
                        selectors[M..N].fill(F::ONE);
                    }

                    // prepare the inputs for the `RecursionTree::prove_node` call
                    let proofs: [PlonkyProof; N] = array::from_fn(|k| proofs_at_level_i[k].clone());
                    let pods_for_node: [POD; N] = array::from_fn(|k| pods[k].clone());

                    let ops_executor_input = ();
                    let ops_executor_output = ();

                    // do the recursive step
                    let start = Instant::now();
                    let new_proof = RT::prove_node(
                        verifier_data.clone(),
                        selectors,
                        pods_for_node,
                        ops_executor_input,
                        ops_executor_output,
                        &proofs,
                    )?;
                    println!(
                        "RecursionTree::prove_node (level: i={}, node: j={}) took: {:?}",
                        i,
                        j,
                        start.elapsed()
                    );

                    // verify the recursive proof
                    let public_inputs = RT::prepare_public_inputs(verifier_data.clone());
                    verifier_data.clone().verify(ProofWithPublicInputs {
                        proof: new_proof.clone(),
                        public_inputs: public_inputs.clone(),
                    })?;

                    // set new_proof for next iteration
                    next_level_proofs.push(new_proof);
                }
                proofs_at_level_i = next_level_proofs.clone();
            }

            assert_eq!(proofs_at_level_i.len(), 1);
            let last_proof = proofs_at_level_i[0].clone();

            // verify the last proof
            let hashes: [HashOut<F>; M + N] = array::from_fn(|k| pods[k].payload.hash_payload());
            let public_inputs = RT::prepare_public_inputs(verifier_data.clone());
            verifier_data.clone().verify(ProofWithPublicInputs {
                proof: last_proof.clone(),
                public_inputs: public_inputs.clone(),
            })?;

            Ok(())
        }
    */
}
