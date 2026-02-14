use super::{Evaluator, EvalError};
use crate::ast::Value;

impl Evaluator {
    /// dot-product: Compute dot product of two vectors
    /// vec1 vec2 dot-product -> scalar
    pub(crate) fn builtin_dot_product(&mut self) -> Result<(), EvalError> {
        let vec2 = self.pop_number_list()?;
        let vec1 = self.pop_number_list()?;

        if vec1.len() != vec2.len() {
            return Err(EvalError::ExecError(format!(
                "dot-product: vectors must have same length ({} vs {})",
                vec1.len(), vec2.len()
            )));
        }

        let result: f64 = vec1.iter().zip(vec2.iter())
            .map(|(a, b)| a * b)
            .sum();

        self.stack.push(Value::Number(result));
        self.last_exit_code = 0;
        Ok(())
    }

    /// magnitude: Compute L2 norm (magnitude) of a vector
    /// vec magnitude -> scalar
    pub(crate) fn builtin_magnitude(&mut self) -> Result<(), EvalError> {
        let vec = self.pop_number_list()?;

        let sum_sq: f64 = vec.iter().map(|x| x * x).sum();
        let result = sum_sq.sqrt();

        self.stack.push(Value::Number(result));
        self.last_exit_code = 0;
        Ok(())
    }

    /// normalize: Convert vector to unit vector
    /// vec normalize -> unit vector
    pub(crate) fn builtin_normalize(&mut self) -> Result<(), EvalError> {
        let vec = self.pop_number_list()?;

        let sum_sq: f64 = vec.iter().map(|x| x * x).sum();
        let mag = sum_sq.sqrt();

        let result: Vec<Value> = if mag == 0.0 {
            vec.iter().map(|_| Value::Number(0.0)).collect()
        } else {
            vec.iter().map(|x| Value::Number(x / mag)).collect()
        };

        self.stack.push(Value::List(result));
        self.last_exit_code = 0;
        Ok(())
    }

    /// cosine-similarity: Compute cosine similarity between two vectors
    /// vec1 vec2 cosine-similarity -> scalar (-1 to 1)
    pub(crate) fn builtin_cosine_similarity(&mut self) -> Result<(), EvalError> {
        let vec2 = self.pop_number_list()?;
        let vec1 = self.pop_number_list()?;

        if vec1.len() != vec2.len() {
            return Err(EvalError::ExecError(format!(
                "cosine-similarity: vectors must have same length ({} vs {})",
                vec1.len(), vec2.len()
            )));
        }

        let dot: f64 = vec1.iter().zip(vec2.iter())
            .map(|(a, b)| a * b)
            .sum();
        let mag1: f64 = vec1.iter().map(|x| x * x).sum::<f64>().sqrt();
        let mag2: f64 = vec2.iter().map(|x| x * x).sum::<f64>().sqrt();

        let result = if mag1 == 0.0 || mag2 == 0.0 {
            0.0
        } else {
            dot / (mag1 * mag2)
        };

        self.stack.push(Value::Number(result));
        self.last_exit_code = 0;
        Ok(())
    }

    /// euclidean-distance: Compute Euclidean distance between two vectors
    /// vec1 vec2 euclidean-distance -> scalar
    pub(crate) fn builtin_euclidean_distance(&mut self) -> Result<(), EvalError> {
        let vec2 = self.pop_number_list()?;
        let vec1 = self.pop_number_list()?;

        if vec1.len() != vec2.len() {
            return Err(EvalError::ExecError(format!(
                "euclidean-distance: vectors must have same length ({} vs {})",
                vec1.len(), vec2.len()
            )));
        }

        let sum_sq: f64 = vec1.iter().zip(vec2.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum();
        let result = sum_sq.sqrt();

        self.stack.push(Value::Number(result));
        self.last_exit_code = 0;
        Ok(())
    }
}
