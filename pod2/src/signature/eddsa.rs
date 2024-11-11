use anyhow::Result;
use num::BigUint;

use plonky2::field::{goldilocks_field::GoldilocksField, types::Field};
use plonky2::hash::poseidon::PoseidonHash;
use plonky2::iop::{
    target::{BoolTarget, Target},
    witness::{PartialWitness, WitnessWrite},
};
use plonky2::plonk::{circuit_builder::CircuitBuilder, config::GenericConfig};

use crate::signature::jubjubcurve::{CircuitBuilderJubjubCurve, JubjubCurveTarget};
use crate::signature::{
    biguint::{BigUintTarget, CircuitBuilderBiguint},
    mod65537::Mod65537Builder,
    schnorr::{SchnorrPublicKey, SchnorrSignature},
};

type GoldF = GoldilocksField;

#[derive(Clone, Debug)]
pub struct MessageHashTarget {
    m: BigUintTarget,
}

#[derive(Clone, Debug)]
pub struct PoseidonOutputTarget {
    h: BigUintTarget,
}

#[derive(Clone, Debug)]
pub struct EddsaSignatureTarget {
    r: JubjubCurveTarget,
    s: BigUintTarget,
}

#[derive(Clone, Debug)]
pub struct EddsaPublicKeyTarget {
    a: JubjubCurveTarget,
}

pub trait EddsaBuilder {
    fn verify_eddsa<C: GenericConfig<2, F = GoldF>> (
        &mut self,
        sig: &EddsaSignatureTarget,
        msg: &MessageHashTarget,
        pk: &EddsaPublicKeyTarget,
        hash: &PoseidonOutputTarget, // will not be needed in final version
    ) -> BoolTarget;

    fn constrain_eddsa<C: GenericConfig<2, F = GoldF>> (
        &mut self,
        sig: &EddsaSignatureTarget,
        msg: &MessageHashTarget,
        pk: &EddsaPublicKeyTarget,
        hash: &PoseidonOutputTarget, // will not be needed in final version
    );
}

impl EddsaBuilder for CircuitBuilder<GoldF, 2> {
    fn verify_eddsa<C: GenericConfig<2, F = GoldF>> (
        &mut self,
        sig: &EddsaSignatureTarget,
        msg: &MessageHashTarget,
        pk: &EddsaPublicKeyTarget,
        hash: &PoseidonOutputTarget, // will not be needed in final version
    ) -> BoolTarget {      
        // WARNING this constrains the sig to be true
        self.verify_jubjub_point(&sig.r);
        self.verify_jubjub_point(&pk.a);
//        HASH TO BE IMPLEMENTED: h should be Poseidon hash of sig.r, pk, msg
//        let h: BigUintTarget = p_hash_to_implement(sig.r.x, sig.r.y, pk.x, pk.y, msg);
        let b8 = self.B8_jubjub_curve();
        let lhs: JubjubCurveTarget = self.mul_scalar(&b8, &sig.s);
        let eight = self.constant_biguint(&BigUint::new(vec![8]));
        let eight_times_pk = self.mul_scalar(&pk.a, &eight);
        let hash_times_pk = self.mul_scalar(&eight_times_pk, &hash.h);
        let rhs = self.add_jubjub_curve(&sig.r, &hash_times_pk);
        self.is_equal_jubjub_curve(&lhs, &rhs)
    }
    
    fn constrain_eddsa<C: GenericConfig<2, F = GoldF>> (
        &mut self,
        sig: &EddsaSignatureTarget,
        msg: &MessageHashTarget,
        pk: &EddsaPublicKeyTarget,
        hash: &PoseidonOutputTarget, // will not be needed in final version
    ) {
        let verification_output = self.verify_eddsa::<C>(sig, msg, pk, hash);
        let true_target = self._true();
        self.connect(verification_output.target, true_target.target);
    }
} 

#[cfg(test)]
mod tests {
    use chrono::Local;
    use num::BigUint;
    use plonky2::field::{goldilocks_field::GoldilocksField, types::Field};
    use plonky2::iop::witness::PartialWitness;
    use plonky2::plonk::circuit_builder::CircuitBuilder;
    use plonky2::plonk::circuit_data::CircuitConfig;
    use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
    use std::str::FromStr;

    use crate::signature::biguint::{BigUintTarget, CircuitBuilderBiguint};
    use crate::signature::eddsa::{
        EddsaBuilder, 
        EddsaPublicKeyTarget, 
        EddsaSignatureTarget,
        MessageHashTarget,
        PoseidonOutputTarget,
    };
    use crate::signature::jubjubcurve::{CircuitBuilderJubjubCurve, JubjubCurveTarget};
    use crate::signature::jubjubfield::{CircuitBuilderJubjubField, JubjubFieldTarget};

    fn u64_to_u32(a: Vec<u64>) -> Vec<u32> {
        a.into_iter().flat_map(|x| vec![x as u32, (x >> 32) as u32]).collect()
    }

    #[test]
    fn test_constrain_sig() {
        dbg!("Start");
        dbg!(Local::now());

        type C = PoseidonGoldilocksConfig;

        let config = CircuitConfig::standard_recursion_config();
        let mut pw: PartialWitness<GoldilocksField> = PartialWitness::new();
        let mut builder = CircuitBuilder::<GoldilocksField, 2>::new(config);

        let px_val = BigUint::new(u64_to_u32(vec![
            14816813974455191966,
            5621129177994019323,
            12490529648725145515,
            2248846258813109077,
        ]));
        let py_val = BigUint::new(u64_to_u32(vec![
            3097575793274345236,
            15962616084270543162,
            8589537196731344667,
            1113026192110903546,
        ]));
        let msg_val = BigUint::new(u64_to_u32(vec![
            14083847773837265618,
            6692605942,
            0,
            0,
        ]));
        let sig_r_x_val = BigUint::new(u64_to_u32(vec![
            3702867781738010923,
            14038445494684018940,
            2926507124369429729,
            656908260954674802,
        ]));
        let sig_r_y_val = BigUint::new(u64_to_u32(vec![
            15389942651936426876,
            9092269394130748824,
            5881620769130576924,
            3001649752140069537,
        ]));
        let sig_s_val = BigUint::new(u64_to_u32(vec![
            6537028312969761294,
            10681752862721662900,
            14992547060379445581,
            133925459310743680,
        ]));
        let hash_val = BigUint::new(u64_to_u32(vec![ // should be computed automatically later
            2709608945152055826,
            8217237346447623338,
            9578917324956230137,
            552733937179642407,
        ]));

        // debugging stuff below
        let l_x_val = BigUint::new(u64_to_u32(vec![
            6769147921293935476,
            15234125309804266507,
            13638367337124659210,
            2737797199439730818,
        ]));
        let l_y_val = BigUint::new(u64_to_u32(vec![
            8193767042881930160,
            8036380828119749438,
            15022032414377487346,
            2478297568907217475,
        ]));
        let htpk_x_val = BigUint::new(u64_to_u32(vec![
            9360752292746738455,
            7922225397280932462,
            2682949893567020685,
            1952627290260948083,
        ]));
        let htpk_y_val = BigUint::new(u64_to_u32(vec![
            11852217244879769029,
            733283315823029871,
            4390128762781367190,
            1336738926330490135,
        ]));

        dbg!(());

        let px = JubjubFieldTarget(builder.constant_biguint(&px_val));
        let py = JubjubFieldTarget(builder.constant_biguint(&py_val));
        let msg = JubjubFieldTarget(builder.constant_biguint(&msg_val));
        let sig_r_x = JubjubFieldTarget(builder.constant_biguint(&sig_r_x_val));
        let sig_r_y = JubjubFieldTarget(builder.constant_biguint(&sig_r_y_val));
        let sig_s = JubjubFieldTarget(builder.constant_biguint(&sig_s_val));
        let hash = JubjubFieldTarget(builder.constant_biguint(&hash_val));

        let pk = JubjubCurveTarget{ x: px, y: py };
        let sig_r = JubjubCurveTarget{ x: sig_r_x, y: sig_r_y };

        let msg = MessageHashTarget{ m: msg.0 };
        let hash = PoseidonOutputTarget{ h: hash.0 };
        let sig = EddsaSignatureTarget{ r: sig_r, s: sig_s.0 };
        let pk = EddsaPublicKeyTarget{ a: pk };

        //debug
        /* 
        let l_x = JubjubFieldTarget(builder.constant_biguint(&l_x_val));
        let l_y = JubjubFieldTarget(builder.constant_biguint(&l_y_val));
        let l = JubjubCurveTarget{ x: l_x, y: l_y };

        let b8 = builder.B8_jubjub_curve();
        let lhs: JubjubCurveTarget = builder.mul_scalar(&b8, &sig.s);
        builder.connect_jubjub_curve(&lhs, &l);

        let htpk_x = JubjubFieldTarget(builder.constant_biguint(&htpk_x_val));
        let htpk_y = JubjubFieldTarget(builder.constant_biguint(&htpk_y_val));
        let htpk = JubjubCurveTarget{ x: htpk_x, y: htpk_y };

        let eight = builder.constant_biguint(&BigUint::new(vec![8]));
        let eight_times_pk = builder.mul_scalar(&pk.a, &eight);
        let hash_times_pk = builder.mul_scalar(&eight_times_pk, &hash.h);
        */

        builder.constrain_eddsa::<C>(&sig, &msg, &pk, &hash);

        dbg!(&pk);
        dbg!(&sig.r);
        dbg!(&sig.s);

        dbg!("look");
        dbg!(Local::now());

        let data = builder.build::<C>();
        dbg!(());
        dbg!(Local::now());
        let proof = data.prove(pw).unwrap();
        dbg!(());
        data.verify(proof);
    }
}
