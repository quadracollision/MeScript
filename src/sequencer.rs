pub(crate) fn pattern_index(values_len: usize, loop_start: usize, step: usize) -> usize {
    if values_len == 0 {
        return 0;
    }
    let loop_start = loop_start.min(values_len - 1);
    if step < loop_start {
        step
    } else {
        loop_start + ((step - loop_start) % (values_len - loop_start).max(1))
    }
}

pub(crate) fn pattern_bool_with_loop(values: &[bool], loop_start: usize, step: usize) -> bool {
    if values.is_empty() {
        false
    } else {
        values[pattern_index(values.len(), loop_start, step)]
    }
}

pub(crate) fn pattern_step_gates(values: &[Vec<bool>], step: usize) -> &[bool] {
    if values.is_empty() {
        &[false]
    } else {
        &values[step % values.len()]
    }
}

pub(crate) fn pattern_step_gates_with_loop(
    values: &[Vec<bool>],
    loop_start: usize,
    step: usize,
) -> &[bool] {
    if values.is_empty() {
        &[false]
    } else {
        &values[pattern_index(values.len(), loop_start, step)]
    }
}

pub(crate) fn pattern_step_holds_with_loop(
    values: &[Vec<usize>],
    loop_start: usize,
    step: usize,
) -> &[usize] {
    if values.is_empty() {
        &[0]
    } else {
        &values[pattern_index(values.len(), loop_start, step)]
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
