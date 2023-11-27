use genevo::mutation::value::{RandomValueMutation, RandomValueMutator};
use once_cell::sync::OnceCell;
use std::collections::{HashMap, HashSet};

use genevo::prelude::*;
use genevo::reinsertion::elitist::ElitistReinserter;
use genevo::selection::truncation::MaximizeSelector;
use genevo::operator::prelude::{SinglePointCrossBreeder, UniformCrossBreeder, MultiPointCrossBreeder};
use genevo::types::fmt::Display;

pub type GroupId = usize;
pub type SubjectId = usize;
pub type LecturerId = usize;

pub const HOURS: usize = 20;

static GROUP_SUBJECTS: OnceCell<Vec<(GroupId, SubjectId)>> = OnceCell::new();
static PROBLEM: OnceCell<Problem> = OnceCell::new();

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Dna(pub (SubjectId, LecturerId, usize));

pub type Genome = Vec<Dna>; // (lecturer, hour) for the corresponding (group, subject) from `GROUP_SUBJECTS`.

#[derive(Debug, Clone)]
struct Problem {
    pub group_requirements: HashMap<GroupId, Vec<(SubjectId, usize)>>, // list of (subject, hours) for each group.
    pub lecturer_requirements: HashMap<LecturerId, usize>,             // hours for each lecturer.
    pub subject_requirements: HashMap<SubjectId, Vec<LecturerId>>, // suitable lecturers for each subject.
}

impl Problem {
    pub fn new(
        group_requirements: HashMap<GroupId, Vec<(SubjectId, usize)>>,
        lecturer_requirements: HashMap<LecturerId, usize>,
        subject_requirements: HashMap<SubjectId, Vec<LecturerId>>,
    ) -> Self {
        Self {
            group_requirements,
            lecturer_requirements,
            subject_requirements,
        }
    }
}

/// The fitness function for `Selection`
impl<'a> FitnessFunction<Genome, i64> for &'a Problem {
    fn fitness_of(&self, genome: &Genome) -> i64 {
        let mut fitness = 0i64;
        let mut used_group_hours: HashSet<(GroupId, usize)> = HashSet::new();
        let mut used_lecturer_hours: HashSet<(LecturerId, usize)> = HashSet::new();
        let mut free_lecturer_hours: HashMap<LecturerId, usize> =
            self.lecturer_requirements.clone();

        for ((group, _subject), (lecturer, hour)) in GROUP_SUBJECTS
            .get()
            .unwrap()
            .iter()
            .zip(genome.iter().map(|x| (x.0 .1, x.0 .2)))
        {
            let satisfies_group = used_group_hours.insert((*group, hour));

            let mut satisfies_lecturer = true;
            if free_lecturer_hours
                .get(&lecturer)
                .copied()
                .unwrap_or_default()
                == 0
            {
                satisfies_lecturer = false;
            }
            if used_lecturer_hours.contains(&(lecturer, hour)) {
                satisfies_lecturer = false;
            }

            if satisfies_lecturer {
                *free_lecturer_hours.get_mut(&lecturer).unwrap() -= 1;
                used_lecturer_hours.insert((lecturer, hour));
            }

            match (satisfies_group, satisfies_lecturer) {
                (true, true) => fitness += 1,
                (false, false) => fitness -= 1,
                _ => {}
            }
        }

        fitness
    }

    fn average(&self, values: &[i64]) -> i64 {
        (values.iter().sum::<i64>() as f32 / values.len() as f32).round() as i64
    }

    fn highest_possible_fitness(&self) -> i64 {
        GROUP_SUBJECTS.get().unwrap().len() as i64
    }

    fn lowest_possible_fitness(&self) -> i64 {
        -(GROUP_SUBJECTS.get().unwrap().len() as i64)
    }
}

#[derive(Debug)]
struct RandomScheduleBuilder(pub Problem);

impl GenomeBuilder<Genome> for RandomScheduleBuilder {
    fn build_genome<R>(&self, _: usize, rng: &mut R) -> Genome
    where
        R: Rng + Sized,
    {
        let group_subjects = GROUP_SUBJECTS.get().unwrap();

        group_subjects
            .iter()
            .map(|(_group, subject)| {
                let lecturers = self.0.subject_requirements.get(subject).unwrap();
                let lecturer = lecturers[rng.gen_range(0..lecturers.len())];
                let hour = rng.gen_range(0..HOURS);

                Dna((*subject, lecturer, hour))
            })
            .collect()
    }
}

impl RandomValueMutation for Dna {
    fn random_mutated<R>(mut value: Self, _min_value: &Self, max_value: &Self, rng: &mut R) -> Self
    where
        R: Rng + Sized,
    {
        value.0 .2 = rng.gen_range(0..max_value.0 .2);

        let lecturers = PROBLEM
            .get()
            .unwrap()
            .subject_requirements
            .get(&value.0 .0)
            .unwrap();
        let index = rng.gen_range(0..lecturers.len());
        value.0 .1 = lecturers[index];

        value
    }
}

fn main() {
    let problem = if std::env::var("SMALL_EXAMPLE").is_ok() {
        let group_requirements = vec![
            (0_usize, vec![(0_usize, 2_usize), (1, 5), (2, 2), (3, 1)]), // 10
            (1_usize, vec![(0_usize, 1_usize), (3, 2), (4, 6), (2, 1)]), // 10
            (2_usize, vec![(0_usize, 1_usize), (2, 8), (3, 1)]),         // 10
        ]
        .into_iter()
        .collect();
        let lecturer_requirements = vec![(0_usize, 6_usize), (1, 6), (2, 10), (3, 4), (4, 4)]
            .into_iter()
            .collect();
        let subject_requirements = vec![
            (0_usize, vec![3_usize]),
            (1, vec![0, 2]),
            (2, vec![0, 1]),
            (3, vec![4]),
            (4, vec![1, 2]),
        ]
        .into_iter()
        .collect();
        Problem::new(
            group_requirements,
            lecturer_requirements,
            subject_requirements,
        )
    } else {
        let str = include_str!("../constraints.json");
        let value: serde_json::Value = serde_json::from_str(str).unwrap();
        let group_requirements = value["groups_subjects_hours"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(group, value)| {
                let reqs: Vec<_> = value
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|obj| {
                        let a = obj["subject"].as_i64().unwrap() as usize;
                        let b = obj["hours"].as_i64().unwrap() as usize;

                        (a, b)
                    })
                    .collect();
                (group, reqs)
            })
            .collect();

        let lecturer_requirements = value["teachers_hours"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(lecturer, value)| {
                let hours = value.as_i64().unwrap() as usize;
                (lecturer, hours)
            })
            .collect();

        let subject_requirements = value["subjects_teachers"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(subject, value)| {
                let reqs: Vec<_> = value
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|obj| obj.as_i64().unwrap() as usize)
                    .collect();
                (subject, reqs)
            })
            .collect();

        Problem::new(
            group_requirements,
            lecturer_requirements,
            subject_requirements,
        )
    };

    let group_subjects: Vec<_> = problem
        .group_requirements
        .iter()
        .map(|(group, subjects)| {
            subjects
                .iter()
                .map(|(subject, hours)| (0..*hours).map(|_| (*group, *subject)))
                .flatten()
        })
        .flatten()
        .collect();
    dbg!(group_subjects.len());
    GROUP_SUBJECTS.set(group_subjects).unwrap();
    PROBLEM.set(problem.clone()).unwrap();


    let initial_population: Population<Genome> = build_population()
        .with_genome_builder(RandomScheduleBuilder(problem.clone()))
        .of_size(200)
        .uniform_at_random();

    let mut simulation = simulate(
        genetic_algorithm()
            .with_evaluation(&problem)
            .with_selection(MaximizeSelector::new(0.85, 20))
            .with_crossover(UniformCrossBreeder::new())
            .with_mutation(RandomValueMutator::new(
                0.2,
                Dna((0, 0, 0)),
                Dna((0, usize::MAX, HOURS - 1)),
            ))
            .with_reinsertion(ElitistReinserter::new(&problem, false, 0.85))
            .with_initial_population(initial_population)
            .build(),
    )
    .until(GenerationLimit::new(100))
    .build();

    let genome = loop {
        let result = simulation.step();

        match result {
            Ok(SimResult::Intermediate(step)) => {
                let evaluated_population = step.result.evaluated_population;
                let best_solution = step.result.best_solution;
                println!(
                    "step: generation: {}, average_fitness: {}, \
                     best fitness: {}, duration: {:?}, processing_time: {:?}",
                    step.iteration,
                    evaluated_population.average_fitness(),
                    best_solution.solution.fitness,
                    step.duration.fmt(),
                    step.processing_time.fmt(),
                );

                if best_solution.solution.fitness == GROUP_SUBJECTS.get().unwrap().len() as i64 {
                    break best_solution.solution.genome;
                }
            }
            Ok(SimResult::Final(step, processing_time, duration, stop_reason)) => {
                let best_solution = step.result.best_solution;
                println!("{}", stop_reason);
                println!(
                    "Final result after {}: generation: {}, \
                     best solution with fitness {} found in generation {}, processing_time: {}",
                    duration.fmt(),
                    step.iteration,
                    best_solution.solution.fitness,
                    best_solution.generation,
                    processing_time.fmt(),
                );

                break best_solution.solution.genome;
            }
            Err(error) => {
                panic!("{}", error);
            }
        }
    };

    let mut res1 = vec![];
    let mut res2 = vec![];
    for ((group, subject), (lecturer, hour)) in GROUP_SUBJECTS
        .get()
        .unwrap()
        .iter()
        .zip(genome.iter().map(|x| (x.0 .1, x.0 .2))) {

        res1.push((group, hour, subject, lecturer));
        res2.push((lecturer, hour, subject, group));

        // println!("group {group}, subject {subject}, lecturer {lecturer}, hour {hour}");
    }
    res1.sort();
    res2.sort();

    println!("Schedule ordered by groups");
    for (group, hour, subject, lecturer) in res1 {
        println!("group {group}, hour {hour}, subject {subject}, lecturer {lecturer}");
    }

    println!("\n\n\nSchedule ordered by lecturers");
    for (lecturer, hour, subject, group) in res2 {
        println!("lecturer {lecturer}, hour {hour}, subject {subject}, group {group}");
    }
}
