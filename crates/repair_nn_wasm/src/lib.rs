//! WASM boundary — Context7 wasm-bindgen constructor pattern.
use repair_nn_core::{RepairNet, WEIGHT_COUNT};
use repair_types::{FeatureVector, OperatorId};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct JsRepairAction {
    pub operator_id: u8,
    pub confidence: f32,
    pub risk: f32,
}

#[wasm_bindgen]
impl JsRepairAction {
    #[wasm_bindgen(getter)]
    pub fn operator_name(&self) -> String {
        OperatorId::from_u8(self.operator_id).as_str().into()
    }
}

#[wasm_bindgen]
pub struct RepairModel {
    net: RepairNet,
}

impl Default for RepairModel {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl RepairModel {
    #[wasm_bindgen(constructor)]
    pub fn new() -> RepairModel {
        RepairModel {
            net: RepairNet::zeros(),
        }
    }

    #[wasm_bindgen(js_name = fromWeights)]
    pub fn from_weights(weights: &[f32]) -> Result<RepairModel, JsValue> {
        RepairNet::from_weights(weights)
            .map(|net| RepairModel { net })
            .map_err(JsValue::from_str)
    }

    #[wasm_bindgen(js_name = predictFromFeatures)]
    pub fn predict_from_features(&self, features: &[f32]) -> Result<JsRepairAction, JsValue> {
        if features.len() != FeatureVector::DIM {
            return Err(JsValue::from_str("feature dim must be 64"));
        }
        let mut fv = FeatureVector::zeros();
        fv.values.copy_from_slice(features);
        let a = self.net.predict(&fv);
        Ok(JsRepairAction {
            operator_id: a.repair_operator as u8,
            confidence: a.confidence,
            risk: a.risk,
        })
    }

    #[wasm_bindgen(js_name = weightCount)]
    pub fn weight_count() -> usize {
        WEIGHT_COUNT
    }

    #[wasm_bindgen(js_name = featureDim)]
    pub fn feature_dim() -> usize {
        FeatureVector::DIM
    }
}
