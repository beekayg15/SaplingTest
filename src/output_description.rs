use std::borrow::Borrow;

use crate::circuit::Output;
use crate::commitment::{Commitment, ValueCommitTrapdoor};
use crate::key_gen::{PublicKey, Signature};
use crate::note::NoteValue;
use ark_crypto_primitives::commitment::pedersen::Randomness;
use ark_crypto_primitives::snark::SNARK;
use ark_ed_on_bls12_381::{EdwardsAffine, EdwardsProjective};
use ark_groth16::{prepare_verifying_key, Groth16, PreparedVerifyingKey, Proof, VerifyingKey};
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystem};
use rand::thread_rng;
pub struct Nullifier(pub [u8; 32]);
pub struct OutputDescription {
    cv: EdwardsAffine,
    cmu: EdwardsAffine,
    epk: EdwardsAffine,
    output_proof: Proof<ark_bls12_381::Bls12_381>,
    verifying_key: PreparedVerifyingKey<ark_bls12_381::Bls12_381>,
}
impl OutputDescription {
    pub fn from_values(
        cv_new: EdwardsAffine,
        note_com: EdwardsAffine,
        epk: PublicKey,
        g_d: EdwardsAffine,
        pk_d: EdwardsAffine,
        note_value: NoteValue,
        rcv: ValueCommitTrapdoor,
        rm_new: Randomness<EdwardsProjective>,
        esk: ark_ed_on_bls12_381::Fr,
    ) -> Self {
        let dummy_output = Output {
            cv_new: None,
            note_com_new: None,
            epk: None,
            g_d: None,
            pk_d: None,
            v_new: None,
            rcv_new: None,
            rcm_new: None,
            esk: None,
            note_com_params: Commitment::setup(),
        };
        let mut rng = thread_rng();
        let output = Output {
            cv_new: Some(cv_new),
            note_com_new: Some(note_com),
            epk: Some(epk.0.into()),
            g_d: Some(g_d),
            pk_d: Some(pk_d),
            v_new: Some(note_value.clone()),
            rcv_new: Some(rcv.clone()),
            rcm_new: Some(rm_new.clone()),
            esk: Some(esk),
            note_com_params: Commitment::setup(),
        };
        let output2 = Output {
            cv_new: Some(cv_new),
            note_com_new: Some(note_com),
            epk: Some(epk.0.into()),
            g_d: Some(g_d),
            pk_d: Some(pk_d),
            v_new: Some(note_value),
            rcv_new: Some(rcv),
            rcm_new: Some(rm_new),
            esk: Some(esk),
            note_com_params: Commitment::setup(),
        };
        let (pk, vk) =
            Groth16::<ark_bls12_381::Bls12_381>::circuit_specific_setup(output, &mut rng).unwrap();
        let proof =
            Groth16::<ark_bls12_381::Bls12_381>::prove(&pk, output2, &mut rng).expect("failed");
        let pvk = prepare_verifying_key(&vk);
        OutputDescription {
            cv: cv_new,
            cmu: note_com,
            epk: epk.0,
            output_proof: proof,
            verifying_key: pvk,
        }
    }
}
#[cfg(test)]
pub mod test {
    use super::*;
    use crate::commitment::homomorphic_pedersen_commitment;
    use crate::group_hash::*;
    use crate::key_gen::Keychain;
    use crate::pedersen_crh::Window;
    use crate::SigningKey;
    use ark_crypto_primitives::commitment::pedersen::{
        Commitment as pdCommit, Randomness as pdRand,
    };
    use ark_crypto_primitives::commitment::CommitmentScheme;
    use ark_ec::AffineRepr;
    use ark_ed_on_bls12_381::Fr;
    use ark_ff::Field;
    use ark_ff::PrimeField;
    use ark_groth16::Groth16;
    use ark_serialize::CanonicalSerialize;
    const SK: SigningKey = &[
        24, 226, 141, 234, 92, 17, 129, 122, 238, 178, 26, 25, 152, 29, 40, 54, 142, 196, 56, 175,
        194, 90, 141, 185, 78, 190, 8, 215, 160, 40, 142, 9,
    ];
    #[test]
    pub fn test_output_description() {
        let kc = Keychain::from(SK);
        let value = NoteValue(10);
        let rcv = ValueCommitTrapdoor::random();
        let cv_new = homomorphic_pedersen_commitment(value.clone(), &rcv);
        let (d, g_d, pk_d) = kc.get_diversified_transmission_address();

        let cm_params = Commitment::setup();
        let rcm = pdRand::<EdwardsProjective>(Fr::from(46));
        let mut note_com_inp = vec![];
        let mut g_d_repr: [u8; 32] = [0; 32];
        <EdwardsAffine as CanonicalSerialize>::serialize_compressed(&g_d, &mut g_d_repr[..])
            .expect("failed");

        let mut pk_d_repr: [u8; 32] = [0; 32];
        <EdwardsAffine as CanonicalSerialize>::serialize_compressed(&pk_d.0, &mut pk_d_repr[..])
            .expect("failed");
        let v_repr = value.0.to_le_bytes();
        note_com_inp.extend(g_d_repr);
        note_com_inp.extend(pk_d_repr);
        note_com_inp.extend(v_repr);
        let note_comm =
            pdCommit::<EdwardsProjective, Window>::commit(&cm_params.params, &note_com_inp, &rcm)
                .expect("failed");
        let esk = Fr::from(5345345);
        let epk = g_d.mul_bigint(esk.0);
        let od = OutputDescription::from_values(
            cv_new,
            note_comm,
            PublicKey(epk.into()),
            g_d,
            pk_d.0,
            value.clone(),
            rcv.clone(),
            rcm.clone(),
            esk.clone(),
        );
        let public_inputs = [od.cmu.x, od.cmu.y, od.cv.x, od.cv.y, od.epk.x, od.epk.y];
        let res = Groth16::<ark_bls12_381::Bls12_381>::verify_with_processed_vk(
            &od.verifying_key,
            &public_inputs,
            &od.output_proof,
        );
        println!("proof: {:?}", od.output_proof);
        println!("res {:?}", res);
        // let output = Output {
        //     cv_new: Some(cv_new),
        //     note_com_new: Some(note_comm),
        //     epk: Some(epk.into()),
        //     g_d: Some(g_d),
        //     pk_d: Some(pk_d.0),
        //     v_new: Some(value),
        //     rcv_new: Some(rcv),
        //     rcm_new: Some(rcm),
        //     esk: Some(esk),
        //     note_com_params: cm_params,
        // };
    }
}
