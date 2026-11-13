//! Capture independent jobs with a bounded pool; return results in input order.
pub fn map<T: Sync, R: Send>(inputs: &[T], run: impl Fn(&T) -> R + Sync) -> Vec<R> {
    const WORKERS: usize = 4;
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..WORKERS.min(inputs.len()))
            .map(|worker| {
                let run = &run;
                scope.spawn(move || {
                    inputs
                        .iter()
                        .enumerate()
                        .skip(worker)
                        .step_by(WORKERS)
                        .map(|(index, input)| (index, run(input)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        let mut ordered: Vec<Option<R>> = (0..inputs.len()).map(|_| None).collect();
        for worker in workers {
            for (index, output) in worker.join().unwrap() {
                ordered[index] = Some(output);
            }
        }
        ordered.into_iter().map(Option::unwrap).collect()
    })
}
