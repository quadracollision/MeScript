pub(crate) fn pattern_f32(values: &[f32], step: usize) -> f32 {
    if values.is_empty() {
        440.0
    } else {
        values[step % values.len()]
    }
}

pub(crate) fn pattern_bool(values: &[bool], step: usize) -> bool {
    if values.is_empty() {
        false
    } else {
        values[step % values.len()]
    }
}

pub(crate) fn pattern_step_gates(values: &[Vec<bool>], step: usize) -> &[bool] {
    if values.is_empty() {
        &[false]
    } else {
        &values[step % values.len()]
    }
}

pub(crate) fn pattern_step_holds(values: &[Vec<usize>], step: usize) -> &[usize] {
    if values.is_empty() {
        &[0]
    } else {
        &values[step % values.len()]
    }
}

pub(crate) fn euclid(pulses: usize, steps: usize, rotation: usize) -> Vec<bool> {
    if steps == 0 {
        return vec![true];
    }
    let pulses = pulses.min(steps);
    (0..steps)
        .map(|idx| {
            let rotated = (idx + steps - (rotation % steps)) % steps;
            ((rotated * pulses) % steps) < pulses
        })
        .collect()
}
