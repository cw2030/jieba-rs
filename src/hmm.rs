use std::collections::HashMap;
use std::cmp::Ordering;

use phf;
use regex::Regex;

use {SplitCaptures};

lazy_static! {
    static ref RE_HAN: Regex = Regex::new(r"([\u{4E00}-\u{9FD5}]+)").unwrap();
    static ref RE_SKIP: Regex = Regex::new(r"([a-zA-Z0-9]+(?:.\d+)?%?)").unwrap();
}

pub type StatusSet = [f64; 4];

#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Clone, Copy)]
pub enum Status {
    B = 0,
    E = 1,
    M = 2,
    S = 3,
}

static PREV_STATUS: [[Status; 2]; 4] = [
    [Status::E, Status::S],  // B
    [Status::B, Status::M],  // E
    [Status::M, Status::B],  // M
    [Status::S, Status::E],  // S
];

include!(concat!(env!("OUT_DIR"), "/hmm_prob.rs"));

const MIN_FLOAT: f64 = -3.14e100;

fn viterbi(sentence: &str, char_indices: &[(usize, char)]) -> Vec<Status> {
    let states = [Status::B, Status::M, Status::E, Status::S];
    #[allow(non_snake_case)]
    let mut V = vec![HashMap::new()];
    let mut path = HashMap::new();
    for y in &states {
        let first_word = if char_indices.len() > 1 {
            &sentence[char_indices[0].0..char_indices[1].0]
        } else {
            &sentence[char_indices[0].0..]
        };
        let prob = INITIAL_PROBS[*y as usize] + EMIT_PROBS[*y as usize].get(first_word).cloned().unwrap_or(MIN_FLOAT);
        V[0].insert(y, prob);
        path.insert(y, vec![y]);
    }
    for t in 1..char_indices.len() {
        V.push(HashMap::new());
        let mut new_path = HashMap::new();
        for y in &states {
            let byte_start = char_indices[t].0;
            let byte_end = if t + 1 < char_indices.len() {
                char_indices[t + 1].0
            } else {
                sentence.len()
            };
            let word = &sentence[byte_start..byte_end];
            let em_prob = EMIT_PROBS[*y as usize].get(word).cloned().unwrap_or(MIN_FLOAT);
            let (prob, state) = PREV_STATUS[*y as usize]
                .iter()
                .map(|y0| {
                    (V[t - 1][y0] + TRANS_PROBS[*y0 as usize].get(*y as usize).cloned().unwrap_or(MIN_FLOAT) + em_prob, *y0)
                })
                .max_by(|x, y| x.partial_cmp(y).unwrap_or(Ordering::Equal))
                .unwrap();
            V[t].insert(y, prob);
            let mut prev_path = path[&state].clone();
            prev_path.push(y);
            new_path.insert(y, prev_path);
        }
        path = new_path;
    }
    let (_prob, state) = [Status::E, Status::S]
        .iter().map(|y| {
            (V[char_indices.len() - 1][y], y)
        })
        .max_by(|x, y| x.partial_cmp(y).unwrap_or(Ordering::Equal))
        .unwrap();
    let best_path: Vec<Status> = path[state].iter().map(|x| **x).collect();
    best_path
}

fn cut_internal<'a>(sentence: &'a str, char_indices: &'a [(usize, char)]) -> Vec<&'a str> {
    let path = viterbi(sentence, char_indices);
    let mut begin = 0;
    let mut next_i = 0;
    let mut words = Vec::new();
    for i in 0..char_indices.len() {
        let state = path[i];
        match state {
            Status::B => begin = i,
            Status::E => {
                let byte_start = char_indices[begin].0;
                let byte_end = if i + 1 < char_indices.len() {
                    char_indices[i + 1].0
                } else {
                    sentence.len()
                };
                words.push(&sentence[byte_start..byte_end]);
                next_i = i + 1;
            },
            Status::S => {
                let byte_start = char_indices[i].0;
                let byte_end = if i + 1 < char_indices.len() {
                    char_indices[i + 1].0
                } else {
                    sentence.len()
                };
                words.push(&sentence[byte_start..byte_end]);
                next_i = i + 1;
            },
            Status::M => { /* do nothing */ },
        }
    }
    if next_i < char_indices.len() {
        let byte_start = char_indices[next_i].0;
        words.push(&sentence[byte_start..]);
    }
    words
}

pub fn cut(sentence: &str) -> Vec<String> {
    let mut words = Vec::new();
    let splitter = SplitCaptures::new(&RE_HAN, sentence);
    for state in splitter {
        let block = state.as_str();
        if block.is_empty() {
            continue;
        }
        if RE_HAN.is_match(block) {
            let char_indices: Vec<(usize, char)> = block.char_indices().collect();
            words.extend(cut_internal(block, &char_indices).into_iter().map(String::from));
        } else {
            let skip_splitter = SplitCaptures::new(&RE_SKIP, block);
            for skip_state in skip_splitter {
                let x = skip_state.as_str();
                if x.is_empty() {
                    continue;
                }
                words.push(x.to_string());
            }
        }
    }
    words
}

#[cfg(test)]
mod tests {
    use super::{viterbi, cut};

    #[test]
    fn test_viterbi() {
        use super::Status::*;

        let sentence = "小明硕士毕业于中国科学院计算所";
        let char_indices: Vec<(usize, char)> = sentence.char_indices().collect();
        let path = viterbi(sentence, &char_indices);
        assert_eq!(path, vec![B, E, B, E, B, M, E, B, E, B, M, E, B, E, S]);
    }

    #[test]
    fn test_hmm_cut() {
        let sentence = "小明硕士毕业于中国科学院计算所";
        let words = cut(sentence);
        assert_eq!(words, vec!["小明", "硕士", "毕业于", "中国", "科学院", "计算", "所"]);
    }
}
