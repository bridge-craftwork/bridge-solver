//! Test suite matching C++ solver test cases

use super::*;

/// Test case structure matching test_cases.txt format
struct TestCase {
    name: &'static str,
    pbn: &'static str,
    trump: usize,
    leader: Seat,
    expected_ns_tricks: u8,
}

const TEST_CASES: &[TestCase] = &[
    // Expected values verified against C++ solver (macroxue/bridge-solver)
    // Both C++ and Rust return NS tricks when West leads
    // C++ output: N  9  9  4  4 means NT W=9, E=9, N=4(EW), S=4(EW)
    // So when W leads: NS makes 9 tricks
    TestCase {
        name: "Test 1: NT West lead",
        pbn: "N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72",
        trump: NOTRUMP,
        leader: WEST,
        expected_ns_tricks: 9,
    },
    TestCase {
        name: "Test 2: NT East lead",
        pbn: "N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72",
        trump: NOTRUMP,
        leader: EAST,
        expected_ns_tricks: 9,
    },
    TestCase {
        name: "Test 3: NT North lead",
        pbn: "N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72",
        trump: NOTRUMP,
        leader: NORTH,
        expected_ns_tricks: 9, // C++ shows 4 (EW tricks), so NS = 13-4 = 9
    },
    TestCase {
        name: "Test 4: Spades West lead",
        pbn: "N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72",
        trump: SPADE,
        leader: WEST,
        expected_ns_tricks: 10, // C++ shows S 10 10 3 3, W lead = 10
    },
    TestCase {
        name: "Test 5: Hearts West lead",
        pbn: "N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72",
        trump: HEART,
        leader: WEST,
        expected_ns_tricks: 8, // C++ shows H 8 8 4 4, W lead = 8
    },
    TestCase {
        name: "Test 6: Diamonds West lead",
        pbn: "N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72",
        trump: DIAMOND,
        leader: WEST,
        expected_ns_tricks: 7, // C++ shows D 7 7 6 6, W lead = 7
    },
    TestCase {
        name: "Test 7: Clubs West lead",
        pbn: "N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72",
        trump: CLUB,
        leader: WEST,
        expected_ns_tricks: 8, // C++ shows C 8 8 5 5, W lead = 8
    },
    TestCase {
        name: "Test 8: Cold 7NT",
        pbn: "N:AKQJ.AKQ.AKQ.AKQ T987.JT9.JT9.JT9 6543.876.876.876 2.5432.5432.5432",
        trump: NOTRUMP,
        leader: WEST,
        expected_ns_tricks: 13,
    },
    TestCase {
        name: "Test 9: NS makes 0",
        pbn: "N:T987.JT9.JT9.JT9 AKQJ.AKQ.AKQ.AKQ 2.5432.5432.5432 6543.876.876.876",
        trump: NOTRUMP,
        leader: WEST,
        expected_ns_tricks: 0,
    },
    TestCase {
        name: "Test 10: Balanced hands",
        pbn: "N:AK32.AK32.K32.32 QJT9.QJT.QJT.QJT 8765.987.987.987 4.654.A654.AK654",
        trump: NOTRUMP,
        leader: WEST,
        expected_ns_tricks: 5, // C++ shows N 5 5 8 8, W lead = 5
    },
];

const UPSTREAM_TEST_CASES: &[TestCase] = &[
    TestCase {
        name: "deal.01 NOTRUMP West",
        pbn: "N:J75.AQT86.J.AK95 92.KJ92.T985.Q72 AKQ864.53.Q42.T3 T3.74.AK763.J864",
        trump: NOTRUMP,
        leader: WEST,
        expected_ns_tricks: 9,
    },
    TestCase {
        name: "deal.01 SPADE West",
        pbn: "N:J75.AQT86.J.AK95 92.KJ92.T985.Q72 AKQ864.53.Q42.T3 T3.74.AK763.J864",
        trump: SPADE,
        leader: WEST,
        expected_ns_tricks: 11,
    },
    TestCase {
        name: "deal.01 HEART West",
        pbn: "N:J75.AQT86.J.AK95 92.KJ92.T985.Q72 AKQ864.53.Q42.T3 T3.74.AK763.J864",
        trump: HEART,
        leader: WEST,
        expected_ns_tricks: 8,
    },
    TestCase {
        name: "deal.01 DIAMOND West",
        pbn: "N:J75.AQT86.J.AK95 92.KJ92.T985.Q72 AKQ864.53.Q42.T3 T3.74.AK763.J864",
        trump: DIAMOND,
        leader: WEST,
        expected_ns_tricks: 6,
    },
    TestCase {
        name: "deal.01 CLUB West",
        pbn: "N:J75.AQT86.J.AK95 92.KJ92.T985.Q72 AKQ864.53.Q42.T3 T3.74.AK763.J864",
        trump: CLUB,
        leader: WEST,
        expected_ns_tricks: 7,
    },
    TestCase {
        name: "deal.02 NOTRUMP West",
        pbn: "N:AT3.T82.AQJ96.76 KQ98754.A.54.T82 62.Q953.T832.AJ4 J.KJ764.K7.KQ953",
        trump: NOTRUMP,
        leader: WEST,
        expected_ns_tricks: 7,
    },
    TestCase {
        name: "deal.02 SPADE West",
        pbn: "N:AT3.T82.AQJ96.76 KQ98754.A.54.T82 62.Q953.T832.AJ4 J.KJ764.K7.KQ953",
        trump: SPADE,
        leader: WEST,
        expected_ns_tricks: 4,
    },
    TestCase {
        name: "deal.02 HEART West",
        pbn: "N:AT3.T82.AQJ96.76 KQ98754.A.54.T82 62.Q953.T832.AJ4 J.KJ764.K7.KQ953",
        trump: HEART,
        leader: WEST,
        expected_ns_tricks: 6,
    },
    TestCase {
        name: "deal.02 DIAMOND West",
        pbn: "N:AT3.T82.AQJ96.76 KQ98754.A.54.T82 62.Q953.T832.AJ4 J.KJ764.K7.KQ953",
        trump: DIAMOND,
        leader: WEST,
        expected_ns_tricks: 8,
    },
    TestCase {
        name: "deal.02 CLUB West",
        pbn: "N:AT3.T82.AQJ96.76 KQ98754.A.54.T82 62.Q953.T832.AJ4 J.KJ764.K7.KQ953",
        trump: CLUB,
        leader: WEST,
        expected_ns_tricks: 4,
    },
    TestCase {
        name: "deal.03 NOTRUMP West",
        pbn: "N:A72.962.KT543.T2 T986.QT3.98.K863 Q54.AJ754.A7.Q94 KJ3.K8.QJ62.AJ75",
        trump: NOTRUMP,
        leader: WEST,
        expected_ns_tricks: 5,
    },
    TestCase {
        name: "deal.03 SPADE West",
        pbn: "N:A72.962.KT543.T2 T986.QT3.98.K863 Q54.AJ754.A7.Q94 KJ3.K8.QJ62.AJ75",
        trump: SPADE,
        leader: WEST,
        expected_ns_tricks: 5,
    },
    TestCase {
        name: "deal.03 HEART West",
        pbn: "N:A72.962.KT543.T2 T986.QT3.98.K863 Q54.AJ754.A7.Q94 KJ3.K8.QJ62.AJ75",
        trump: HEART,
        leader: WEST,
        expected_ns_tricks: 8,
    },
    TestCase {
        name: "deal.03 DIAMOND West",
        pbn: "N:A72.962.KT543.T2 T986.QT3.98.K863 Q54.AJ754.A7.Q94 KJ3.K8.QJ62.AJ75",
        trump: DIAMOND,
        leader: WEST,
        expected_ns_tricks: 7,
    },
    TestCase {
        name: "deal.03 CLUB West",
        pbn: "N:A72.962.KT543.T2 T986.QT3.98.K863 Q54.AJ754.A7.Q94 KJ3.K8.QJ62.AJ75",
        trump: CLUB,
        leader: WEST,
        expected_ns_tricks: 4,
    },
    TestCase {
        name: "deal.04 NOTRUMP West",
        pbn: "N:Q7652.T54.9642.A AJ9.J732.K3.JT86 KT3.A986.AQT8.43 84.KQ.J75.KQ9752",
        trump: NOTRUMP,
        leader: WEST,
        expected_ns_tricks: 5,
    },
    TestCase {
        name: "deal.04 SPADE West",
        pbn: "N:Q7652.T54.9642.A AJ9.J732.K3.JT86 KT3.A986.AQT8.43 84.KQ.J75.KQ9752",
        trump: SPADE,
        leader: WEST,
        expected_ns_tricks: 9,
    },
    TestCase {
        name: "deal.04 HEART West",
        pbn: "N:Q7652.T54.9642.A AJ9.J732.K3.JT86 KT3.A986.AQT8.43 84.KQ.J75.KQ9752",
        trump: HEART,
        leader: WEST,
        expected_ns_tricks: 8,
    },
    TestCase {
        name: "deal.04 DIAMOND West",
        pbn: "N:Q7652.T54.9642.A AJ9.J732.K3.JT86 KT3.A986.AQT8.43 84.KQ.J75.KQ9752",
        trump: DIAMOND,
        leader: WEST,
        expected_ns_tricks: 10,
    },
    TestCase {
        name: "deal.04 CLUB West",
        pbn: "N:Q7652.T54.9642.A AJ9.J732.K3.JT86 KT3.A986.AQT8.43 84.KQ.J75.KQ9752",
        trump: CLUB,
        leader: WEST,
        expected_ns_tricks: 5,
    },
    TestCase {
        name: "deal.05 NOTRUMP West",
        pbn: "N:752.J7.JT.AQT832 94.KT842.832.KJ9 KQT63.AQ95.65.54 AJ8.63.AKQ974.76",
        trump: NOTRUMP,
        leader: WEST,
        expected_ns_tricks: 4,
    },
    TestCase {
        name: "deal.05 SPADE West",
        pbn: "N:752.J7.JT.AQT832 94.KT842.832.KJ9 KQT63.AQ95.65.54 AJ8.63.AKQ974.76",
        trump: SPADE,
        leader: WEST,
        expected_ns_tricks: 7,
    },
    TestCase {
        name: "deal.05 HEART West",
        pbn: "N:752.J7.JT.AQT832 94.KT842.832.KJ9 KQT63.AQ95.65.54 AJ8.63.AKQ974.76",
        trump: HEART,
        leader: WEST,
        expected_ns_tricks: 6,
    },
    TestCase {
        name: "deal.05 DIAMOND West",
        pbn: "N:752.J7.JT.AQT832 94.KT842.832.KJ9 KQT63.AQ95.65.54 AJ8.63.AKQ974.76",
        trump: DIAMOND,
        leader: WEST,
        expected_ns_tricks: 3,
    },
    TestCase {
        name: "deal.05 CLUB West",
        pbn: "N:752.J7.JT.AQT832 94.KT842.832.KJ9 KQT63.AQ95.65.54 AJ8.63.AKQ974.76",
        trump: CLUB,
        leader: WEST,
        expected_ns_tricks: 7,
    },
    TestCase {
        name: "deal.06 NOTRUMP West",
        pbn: "N:KT.AKJ83.KQT73.Q AJ7.QT754.42.985 86543.-.J95.AJT32 Q92.962.A86.K764",
        trump: NOTRUMP,
        leader: WEST,
        expected_ns_tricks: 7,
    },
    TestCase {
        name: "deal.06 SPADE West",
        pbn: "N:KT.AKJ83.KQT73.Q AJ7.QT754.42.985 86543.-.J95.AJT32 Q92.962.A86.K764",
        trump: SPADE,
        leader: WEST,
        expected_ns_tricks: 8,
    },
    TestCase {
        name: "deal.06 HEART West",
        pbn: "N:KT.AKJ83.KQT73.Q AJ7.QT754.42.985 86543.-.J95.AJT32 Q92.962.A86.K764",
        trump: HEART,
        leader: WEST,
        expected_ns_tricks: 7,
    },
    TestCase {
        name: "deal.06 DIAMOND West",
        pbn: "N:KT.AKJ83.KQT73.Q AJ7.QT754.42.985 86543.-.J95.AJT32 Q92.962.A86.K764",
        trump: DIAMOND,
        leader: WEST,
        expected_ns_tricks: 10,
    },
    TestCase {
        name: "deal.06 CLUB West",
        pbn: "N:KT.AKJ83.KQT73.Q AJ7.QT754.42.985 86543.-.J95.AJT32 Q92.962.A86.K764",
        trump: CLUB,
        leader: WEST,
        expected_ns_tricks: 7,
    },
    TestCase {
        name: "deal.07 NOTRUMP West",
        pbn: "N:AQJ52.A953.Q42.Q T6.J76.-.AKJT9865 K984.KT8.K983.43 73.Q42.AJT765.72",
        trump: NOTRUMP,
        leader: WEST,
        expected_ns_tricks: 4,
    },
    TestCase {
        name: "deal.07 SPADE West",
        pbn: "N:AQJ52.A953.Q42.Q T6.J76.-.AKJT9865 K984.KT8.K983.43 73.Q42.AJT765.72",
        trump: SPADE,
        leader: WEST,
        expected_ns_tricks: 9,
    },
    TestCase {
        name: "deal.07 HEART West",
        pbn: "N:AQJ52.A953.Q42.Q T6.J76.-.AKJT9865 K984.KT8.K983.43 73.Q42.AJT765.72",
        trump: HEART,
        leader: WEST,
        expected_ns_tricks: 9,
    },
    TestCase {
        name: "deal.07 DIAMOND West",
        pbn: "N:AQJ52.A953.Q42.Q T6.J76.-.AKJT9865 K984.KT8.K983.43 73.Q42.AJT765.72",
        trump: DIAMOND,
        leader: WEST,
        expected_ns_tricks: 8,
    },
    TestCase {
        name: "deal.07 CLUB West",
        pbn: "N:AQJ52.A953.Q42.Q T6.J76.-.AKJT9865 K984.KT8.K983.43 73.Q42.AJT765.72",
        trump: CLUB,
        leader: WEST,
        expected_ns_tricks: 4,
    },
    TestCase {
        name: "deal.08 NOTRUMP West",
        pbn: "N:Q5.JT6.AQJ96.954 K842.752.85.KJT7 J63.KQ843.743.A3 AT97.A9.KT2.Q862",
        trump: NOTRUMP,
        leader: WEST,
        expected_ns_tricks: 7,
    },
    TestCase {
        name: "deal.08 SPADE West",
        pbn: "N:Q5.JT6.AQJ96.954 K842.752.85.KJT7 J63.KQ843.743.A3 AT97.A9.KT2.Q862",
        trump: SPADE,
        leader: WEST,
        expected_ns_tricks: 5,
    },
    TestCase {
        name: "deal.08 HEART West",
        pbn: "N:Q5.JT6.AQJ96.954 K842.752.85.KJT7 J63.KQ843.743.A3 AT97.A9.KT2.Q862",
        trump: HEART,
        leader: WEST,
        expected_ns_tricks: 9,
    },
    TestCase {
        name: "deal.08 DIAMOND West",
        pbn: "N:Q5.JT6.AQJ96.954 K842.752.85.KJT7 J63.KQ843.743.A3 AT97.A9.KT2.Q862",
        trump: DIAMOND,
        leader: WEST,
        expected_ns_tricks: 9,
    },
    TestCase {
        name: "deal.08 CLUB West",
        pbn: "N:Q5.JT6.AQJ96.954 K842.752.85.KJT7 J63.KQ843.743.A3 AT97.A9.KT2.Q862",
        trump: CLUB,
        leader: WEST,
        expected_ns_tricks: 5,
    },
    TestCase {
        name: "deal.09 NOTRUMP West",
        pbn: "N:J.J74.KJ8762.Q82 A532.952.953.J96 8764.AT8.A.AT753 KQT9.KQ63.QT4.K4",
        trump: NOTRUMP,
        leader: WEST,
        expected_ns_tricks: 7,
    },
    TestCase {
        name: "deal.09 SPADE West",
        pbn: "N:J.J74.KJ8762.Q82 A532.952.953.J96 8764.AT8.A.AT753 KQT9.KQ63.QT4.K4",
        trump: SPADE,
        leader: WEST,
        expected_ns_tricks: 6,
    },
    TestCase {
        name: "deal.09 HEART West",
        pbn: "N:J.J74.KJ8762.Q82 A532.952.953.J96 8764.AT8.A.AT753 KQT9.KQ63.QT4.K4",
        trump: HEART,
        leader: WEST,
        expected_ns_tricks: 6,
    },
    TestCase {
        name: "deal.09 DIAMOND West",
        pbn: "N:J.J74.KJ8762.Q82 A532.952.953.J96 8764.AT8.A.AT753 KQT9.KQ63.QT4.K4",
        trump: DIAMOND,
        leader: WEST,
        expected_ns_tricks: 9,
    },
    TestCase {
        name: "deal.09 CLUB West",
        pbn: "N:J.J74.KJ8762.Q82 A532.952.953.J96 8764.AT8.A.AT753 KQT9.KQ63.QT4.K4",
        trump: CLUB,
        leader: WEST,
        expected_ns_tricks: 8,
    },
    TestCase {
        name: "deal.10 NOTRUMP West",
        pbn: "N:643.75.63.AJT432 T2.J862.KQ9754.8 KQJ95.T9.A82.Q96 A87.AKQ43.JT.K75",
        trump: NOTRUMP,
        leader: WEST,
        expected_ns_tricks: 7,
    },
    TestCase {
        name: "deal.10 SPADE West",
        pbn: "N:643.75.63.AJT432 T2.J862.KQ9754.8 KQJ95.T9.A82.Q96 A87.AKQ43.JT.K75",
        trump: SPADE,
        leader: WEST,
        expected_ns_tricks: 8,
    },
    TestCase {
        name: "deal.10 HEART West",
        pbn: "N:643.75.63.AJT432 T2.J862.KQ9754.8 KQJ95.T9.A82.Q96 A87.AKQ43.JT.K75",
        trump: HEART,
        leader: WEST,
        expected_ns_tricks: 2,
    },
    TestCase {
        name: "deal.10 DIAMOND West",
        pbn: "N:643.75.63.AJT432 T2.J862.KQ9754.8 KQJ95.T9.A82.Q96 A87.AKQ43.JT.K75",
        trump: DIAMOND,
        leader: WEST,
        expected_ns_tricks: 2,
    },
    TestCase {
        name: "deal.10 CLUB West",
        pbn: "N:643.75.63.AJT432 T2.J862.KQ9754.8 KQJ95.T9.A82.Q96 A87.AKQ43.JT.K75",
        trump: CLUB,
        leader: WEST,
        expected_ns_tricks: 9,
    },
    TestCase {
        name: "deal.11 NOTRUMP West",
        pbn: "N:43.A3.KJT987.AJ4 5.J9865.5.KQ9752 QT62.KQ.A64.T863 AKJ987.T742.Q32.-",
        trump: NOTRUMP,
        leader: WEST,
        expected_ns_tricks: 9,
    },
    TestCase {
        name: "deal.11 SPADE West",
        pbn: "N:43.A3.KJT987.AJ4 5.J9865.5.KQ9752 QT62.KQ.A64.T863 AKJ987.T742.Q32.-",
        trump: SPADE,
        leader: WEST,
        expected_ns_tricks: 6,
    },
    TestCase {
        name: "deal.11 HEART West",
        pbn: "N:43.A3.KJT987.AJ4 5.J9865.5.KQ9752 QT62.KQ.A64.T863 AKJ987.T742.Q32.-",
        trump: HEART,
        leader: WEST,
        expected_ns_tricks: 3,
    },
    TestCase {
        name: "deal.11 DIAMOND West",
        pbn: "N:43.A3.KJT987.AJ4 5.J9865.5.KQ9752 QT62.KQ.A64.T863 AKJ987.T742.Q32.-",
        trump: DIAMOND,
        leader: WEST,
        expected_ns_tricks: 9,
    },
    TestCase {
        name: "deal.11 CLUB West",
        pbn: "N:43.A3.KJT987.AJ4 5.J9865.5.KQ9752 QT62.KQ.A64.T863 AKJ987.T742.Q32.-",
        trump: CLUB,
        leader: WEST,
        expected_ns_tricks: 7,
    },
    TestCase {
        name: "deal.12 NOTRUMP West",
        pbn: "N:AJ742.KT54.3.A95 KQT963.98.754.JT -.AQ632.KT2.Q8764 85.J7.AQJ986.K32",
        trump: NOTRUMP,
        leader: WEST,
        expected_ns_tricks: 8,
    },
    TestCase {
        name: "deal.12 SPADE West",
        pbn: "N:AJ742.KT54.3.A95 KQT963.98.754.JT -.AQ632.KT2.Q8764 85.J7.AQJ986.K32",
        trump: SPADE,
        leader: WEST,
        expected_ns_tricks: 6,
    },
    TestCase {
        name: "deal.12 HEART West",
        pbn: "N:AJ742.KT54.3.A95 KQT963.98.754.JT -.AQ632.KT2.Q8764 85.J7.AQJ986.K32",
        trump: HEART,
        leader: WEST,
        expected_ns_tricks: 11,
    },
    TestCase {
        name: "deal.12 DIAMOND West",
        pbn: "N:AJ742.KT54.3.A95 KQT963.98.754.JT -.AQ632.KT2.Q8764 85.J7.AQJ986.K32",
        trump: DIAMOND,
        leader: WEST,
        expected_ns_tricks: 5,
    },
    TestCase {
        name: "deal.12 CLUB West",
        pbn: "N:AJ742.KT54.3.A95 KQT963.98.754.JT -.AQ632.KT2.Q8764 85.J7.AQJ986.K32",
        trump: CLUB,
        leader: WEST,
        expected_ns_tricks: 11,
    },
    TestCase {
        name: "deal.13 NOTRUMP West",
        pbn: "N:KT98.AQJ73.K5.82 AQ642.862.T92.93 53.95.Q864.AJ764 J7.KT4.AJ73.KQT5",
        trump: NOTRUMP,
        leader: WEST,
        expected_ns_tricks: 7,
    },
    TestCase {
        name: "deal.13 SPADE West",
        pbn: "N:KT98.AQJ73.K5.82 AQ642.862.T92.93 53.95.Q864.AJ764 J7.KT4.AJ73.KQT5",
        trump: SPADE,
        leader: WEST,
        expected_ns_tricks: 6,
    },
    TestCase {
        name: "deal.13 HEART West",
        pbn: "N:KT98.AQJ73.K5.82 AQ642.862.T92.93 53.95.Q864.AJ764 J7.KT4.AJ73.KQT5",
        trump: HEART,
        leader: WEST,
        expected_ns_tricks: 8,
    },
    TestCase {
        name: "deal.13 DIAMOND West",
        pbn: "N:KT98.AQJ73.K5.82 AQ642.862.T92.93 53.95.Q864.AJ764 J7.KT4.AJ73.KQT5",
        trump: DIAMOND,
        leader: WEST,
        expected_ns_tricks: 5,
    },
    TestCase {
        name: "deal.13 CLUB West",
        pbn: "N:KT98.AQJ73.K5.82 AQ642.862.T92.93 53.95.Q864.AJ764 J7.KT4.AJ73.KQT5",
        trump: CLUB,
        leader: WEST,
        expected_ns_tricks: 7,
    },
    TestCase {
        name: "deal.14 NOTRUMP West",
        pbn: "N:QT32.Q92.T.A9852 A75.JT3.943.KJT6 K984.A765.AQ76.4 J6.K84.KJ852.Q73",
        trump: NOTRUMP,
        leader: WEST,
        expected_ns_tricks: 7,
    },
    TestCase {
        name: "deal.14 SPADE West",
        pbn: "N:QT32.Q92.T.A9852 A75.JT3.943.KJT6 K984.A765.AQ76.4 J6.K84.KJ852.Q73",
        trump: SPADE,
        leader: WEST,
        expected_ns_tricks: 9,
    },
    TestCase {
        name: "deal.14 HEART West",
        pbn: "N:QT32.Q92.T.A9852 A75.JT3.943.KJT6 K984.A765.AQ76.4 J6.K84.KJ852.Q73",
        trump: HEART,
        leader: WEST,
        expected_ns_tricks: 9,
    },
    TestCase {
        name: "deal.14 DIAMOND West",
        pbn: "N:QT32.Q92.T.A9852 A75.JT3.943.KJT6 K984.A765.AQ76.4 J6.K84.KJ852.Q73",
        trump: DIAMOND,
        leader: WEST,
        expected_ns_tricks: 7,
    },
    TestCase {
        name: "deal.14 CLUB West",
        pbn: "N:QT32.Q92.T.A9852 A75.JT3.943.KJT6 K984.A765.AQ76.4 J6.K84.KJ852.Q73",
        trump: CLUB,
        leader: WEST,
        expected_ns_tricks: 7,
    },
    TestCase {
        name: "deal.15 NOTRUMP West",
        pbn: "N:Q83.53.94.AKQJT7 A64.KJ92.A82.964 T952.T8.KQT5.853 KJ7.AQ764.J763.2",
        trump: NOTRUMP,
        leader: WEST,
        expected_ns_tricks: 5,
    },
    TestCase {
        name: "deal.15 SPADE West",
        pbn: "N:Q83.53.94.AKQJT7 A64.KJ92.A82.964 T952.T8.KQT5.853 KJ7.AQ764.J763.2",
        trump: SPADE,
        leader: WEST,
        expected_ns_tricks: 6,
    },
    TestCase {
        name: "deal.15 HEART West",
        pbn: "N:Q83.53.94.AKQJT7 A64.KJ92.A82.964 T952.T8.KQT5.853 KJ7.AQ764.J763.2",
        trump: HEART,
        leader: WEST,
        expected_ns_tricks: 3,
    },
    TestCase {
        name: "deal.15 DIAMOND West",
        pbn: "N:Q83.53.94.AKQJT7 A64.KJ92.A82.964 T952.T8.KQT5.853 KJ7.AQ764.J763.2",
        trump: DIAMOND,
        leader: WEST,
        expected_ns_tricks: 5,
    },
    TestCase {
        name: "deal.15 CLUB West",
        pbn: "N:Q83.53.94.AKQJT7 A64.KJ92.A82.964 T952.T8.KQT5.853 KJ7.AQ764.J763.2",
        trump: CLUB,
        leader: WEST,
        expected_ns_tricks: 8,
    },
    TestCase {
        name: "deal.16 NOTRUMP West",
        pbn: "N:K97.KJ4.K96.KJ96 AJT82.Q32.T3.Q53 4.T98.A8752.8742 Q653.A765.QJ4.AT",
        trump: NOTRUMP,
        leader: WEST,
        expected_ns_tricks: 4,
    },
    TestCase {
        name: "deal.16 SPADE West",
        pbn: "N:K97.KJ4.K96.KJ96 AJT82.Q32.T3.Q53 4.T98.A8752.8742 Q653.A765.QJ4.AT",
        trump: SPADE,
        leader: WEST,
        expected_ns_tricks: 3,
    },
    TestCase {
        name: "deal.16 HEART West",
        pbn: "N:K97.KJ4.K96.KJ96 AJT82.Q32.T3.Q53 4.T98.A8752.8742 Q653.A765.QJ4.AT",
        trump: HEART,
        leader: WEST,
        expected_ns_tricks: 4,
    },
    TestCase {
        name: "deal.16 DIAMOND West",
        pbn: "N:K97.KJ4.K96.KJ96 AJT82.Q32.T3.Q53 4.T98.A8752.8742 Q653.A765.QJ4.AT",
        trump: DIAMOND,
        leader: WEST,
        expected_ns_tricks: 7,
    },
    TestCase {
        name: "deal.16 CLUB West",
        pbn: "N:K97.KJ4.K96.KJ96 AJT82.Q32.T3.Q53 4.T98.A8752.8742 Q653.A765.QJ4.AT",
        trump: CLUB,
        leader: WEST,
        expected_ns_tricks: 7,
    },
    TestCase {
        name: "deal.17 NOTRUMP West",
        pbn: "N:965.6.QT875.KQJ6 T84.AT42.AJ9.742 KQJ7.K953.K62.A8 A32.QJ87.43.T953",
        trump: NOTRUMP,
        leader: WEST,
        expected_ns_tricks: 8,
    },
    TestCase {
        name: "deal.17 SPADE West",
        pbn: "N:965.6.QT875.KQJ6 T84.AT42.AJ9.742 KQJ7.K953.K62.A8 A32.QJ87.43.T953",
        trump: SPADE,
        leader: WEST,
        expected_ns_tricks: 9,
    },
    TestCase {
        name: "deal.17 HEART West",
        pbn: "N:965.6.QT875.KQJ6 T84.AT42.AJ9.742 KQJ7.K953.K62.A8 A32.QJ87.43.T953",
        trump: HEART,
        leader: WEST,
        expected_ns_tricks: 7,
    },
    TestCase {
        name: "deal.17 DIAMOND West",
        pbn: "N:965.6.QT875.KQJ6 T84.AT42.AJ9.742 KQJ7.K953.K62.A8 A32.QJ87.43.T953",
        trump: DIAMOND,
        leader: WEST,
        expected_ns_tricks: 9,
    },
    TestCase {
        name: "deal.17 CLUB West",
        pbn: "N:965.6.QT875.KQJ6 T84.AT42.AJ9.742 KQJ7.K953.K62.A8 A32.QJ87.43.T953",
        trump: CLUB,
        leader: WEST,
        expected_ns_tricks: 9,
    },
    TestCase {
        name: "deal.18 NOTRUMP West",
        pbn: "N:QT764.AQ6.Q54.J3 98.KT32.J82.K942 A53.87.AT963.Q65 KJ2.J954.K7.AT87",
        trump: NOTRUMP,
        leader: WEST,
        expected_ns_tricks: 6,
    },
    TestCase {
        name: "deal.18 SPADE West",
        pbn: "N:QT764.AQ6.Q54.J3 98.KT32.J82.K942 A53.87.AT963.Q65 KJ2.J954.K7.AT87",
        trump: SPADE,
        leader: WEST,
        expected_ns_tricks: 8,
    },
    TestCase {
        name: "deal.18 HEART West",
        pbn: "N:QT764.AQ6.Q54.J3 98.KT32.J82.K942 A53.87.AT963.Q65 KJ2.J954.K7.AT87",
        trump: HEART,
        leader: WEST,
        expected_ns_tricks: 4,
    },
    TestCase {
        name: "deal.18 DIAMOND West",
        pbn: "N:QT764.AQ6.Q54.J3 98.KT32.J82.K942 A53.87.AT963.Q65 KJ2.J954.K7.AT87",
        trump: DIAMOND,
        leader: WEST,
        expected_ns_tricks: 8,
    },
    TestCase {
        name: "deal.18 CLUB West",
        pbn: "N:QT764.AQ6.Q54.J3 98.KT32.J82.K942 A53.87.AT963.Q65 KJ2.J954.K7.AT87",
        trump: CLUB,
        leader: WEST,
        expected_ns_tricks: 4,
    },
    TestCase {
        name: "deal.19 NOTRUMP West",
        pbn: "N:Q97632.A87.J4.Q7 -.KJT94.AQ93.J654 KT8.Q63.KT87.KT8 AJ54.52.652.A932",
        trump: NOTRUMP,
        leader: WEST,
        expected_ns_tricks: 8,
    },
    TestCase {
        name: "deal.19 SPADE West",
        pbn: "N:Q97632.A87.J4.Q7 -.KJT94.AQ93.J654 KT8.Q63.KT87.KT8 AJ54.52.652.A932",
        trump: SPADE,
        leader: WEST,
        expected_ns_tricks: 8,
    },
    TestCase {
        name: "deal.19 HEART West",
        pbn: "N:Q97632.A87.J4.Q7 -.KJT94.AQ93.J654 KT8.Q63.KT87.KT8 AJ54.52.652.A932",
        trump: HEART,
        leader: WEST,
        expected_ns_tricks: 6,
    },
    TestCase {
        name: "deal.19 DIAMOND West",
        pbn: "N:Q97632.A87.J4.Q7 -.KJT94.AQ93.J654 KT8.Q63.KT87.KT8 AJ54.52.652.A932",
        trump: DIAMOND,
        leader: WEST,
        expected_ns_tricks: 6,
    },
    TestCase {
        name: "deal.19 CLUB West",
        pbn: "N:Q97632.A87.J4.Q7 -.KJT94.AQ93.J654 KT8.Q63.KT87.KT8 AJ54.52.652.A932",
        trump: CLUB,
        leader: WEST,
        expected_ns_tricks: 4,
    },
    TestCase {
        name: "deal.20 NOTRUMP West",
        pbn: "N:2.A765.AKT964.A4 J75.T942.QJ82.63 QT3.KJ3.5.KT9852 AK9864.Q8.73.QJ7",
        trump: NOTRUMP,
        leader: WEST,
        expected_ns_tricks: 8,
    },
    TestCase {
        name: "deal.20 SPADE West",
        pbn: "N:2.A765.AKT964.A4 J75.T942.QJ82.63 QT3.KJ3.5.KT9852 AK9864.Q8.73.QJ7",
        trump: SPADE,
        leader: WEST,
        expected_ns_tricks: 7,
    },
    TestCase {
        name: "deal.20 HEART West",
        pbn: "N:2.A765.AKT964.A4 J75.T942.QJ82.63 QT3.KJ3.5.KT9852 AK9864.Q8.73.QJ7",
        trump: HEART,
        leader: WEST,
        expected_ns_tricks: 10,
    },
    TestCase {
        name: "deal.20 DIAMOND West",
        pbn: "N:2.A765.AKT964.A4 J75.T942.QJ82.63 QT3.KJ3.5.KT9852 AK9864.Q8.73.QJ7",
        trump: DIAMOND,
        leader: WEST,
        expected_ns_tricks: 10,
    },
    TestCase {
        name: "deal.20 CLUB West",
        pbn: "N:2.A765.AKT964.A4 J75.T942.QJ82.63 QT3.KJ3.5.KT9852 AK9864.Q8.73.QJ7",
        trump: CLUB,
        leader: WEST,
        expected_ns_tricks: 11,
    },
    TestCase {
        name: "deal.21 NOTRUMP West",
        pbn: "N:J82.9.AK42.KQT86 AQ5.A6543.85.A97 KT4.KJ82.J97.J32 9763.QT7.QT63.54",
        trump: NOTRUMP,
        leader: WEST,
        expected_ns_tricks: 9,
    },
    TestCase {
        name: "deal.21 SPADE West",
        pbn: "N:J82.9.AK42.KQT86 AQ5.A6543.85.A97 KT4.KJ82.J97.J32 9763.QT7.QT63.54",
        trump: SPADE,
        leader: WEST,
        expected_ns_tricks: 8,
    },
    TestCase {
        name: "deal.21 HEART West",
        pbn: "N:J82.9.AK42.KQT86 AQ5.A6543.85.A97 KT4.KJ82.J97.J32 9763.QT7.QT63.54",
        trump: HEART,
        leader: WEST,
        expected_ns_tricks: 7,
    },
    TestCase {
        name: "deal.21 DIAMOND West",
        pbn: "N:J82.9.AK42.KQT86 AQ5.A6543.85.A97 KT4.KJ82.J97.J32 9763.QT7.QT63.54",
        trump: DIAMOND,
        leader: WEST,
        expected_ns_tricks: 9,
    },
    TestCase {
        name: "deal.21 CLUB West",
        pbn: "N:J82.9.AK42.KQT86 AQ5.A6543.85.A97 KT4.KJ82.J97.J32 9763.QT7.QT63.54",
        trump: CLUB,
        leader: WEST,
        expected_ns_tricks: 10,
    },
    TestCase {
        name: "deal.22 NOTRUMP West",
        pbn: "N:J4.A985.Q9854.KT QT6.43.AKJT3.J42 A752.KQJ7.76.A96 K983.T62.2.Q8753",
        trump: NOTRUMP,
        leader: WEST,
        expected_ns_tricks: 7,
    },
    TestCase {
        name: "deal.22 SPADE West",
        pbn: "N:J4.A985.Q9854.KT QT6.43.AKJT3.J42 A752.KQJ7.76.A96 K983.T62.2.Q8753",
        trump: SPADE,
        leader: WEST,
        expected_ns_tricks: 6,
    },
    TestCase {
        name: "deal.22 HEART West",
        pbn: "N:J4.A985.Q9854.KT QT6.43.AKJT3.J42 A752.KQJ7.76.A96 K983.T62.2.Q8753",
        trump: HEART,
        leader: WEST,
        expected_ns_tricks: 9,
    },
    TestCase {
        name: "deal.22 DIAMOND West",
        pbn: "N:J4.A985.Q9854.KT QT6.43.AKJT3.J42 A752.KQJ7.76.A96 K983.T62.2.Q8753",
        trump: DIAMOND,
        leader: WEST,
        expected_ns_tricks: 7,
    },
    TestCase {
        name: "deal.22 CLUB West",
        pbn: "N:J4.A985.Q9854.KT QT6.43.AKJT3.J42 A752.KQJ7.76.A96 K983.T62.2.Q8753",
        trump: CLUB,
        leader: WEST,
        expected_ns_tricks: 5,
    },
    TestCase {
        name: "deal.23 NOTRUMP West",
        pbn: "N:Q832.KQJ5.853.97 J96.T8642.-.AQT32 5.3.AKJ9762.J654 AKT74.A97.QT4.K8",
        trump: NOTRUMP,
        leader: WEST,
        expected_ns_tricks: 4,
    },
    TestCase {
        name: "deal.23 SPADE West",
        pbn: "N:Q832.KQJ5.853.97 J96.T8642.-.AQT32 5.3.AKJ9762.J654 AKT74.A97.QT4.K8",
        trump: SPADE,
        leader: WEST,
        expected_ns_tricks: 3,
    },
    TestCase {
        name: "deal.23 HEART West",
        pbn: "N:Q832.KQJ5.853.97 J96.T8642.-.AQT32 5.3.AKJ9762.J654 AKT74.A97.QT4.K8",
        trump: HEART,
        leader: WEST,
        expected_ns_tricks: 3,
    },
    TestCase {
        name: "deal.23 DIAMOND West",
        pbn: "N:Q832.KQJ5.853.97 J96.T8642.-.AQT32 5.3.AKJ9762.J654 AKT74.A97.QT4.K8",
        trump: DIAMOND,
        leader: WEST,
        expected_ns_tricks: 7,
    },
    TestCase {
        name: "deal.23 CLUB West",
        pbn: "N:Q832.KQJ5.853.97 J96.T8642.-.AQT32 5.3.AKJ9762.J654 AKT74.A97.QT4.K8",
        trump: CLUB,
        leader: WEST,
        expected_ns_tricks: 4,
    },
    TestCase {
        name: "deal.24 NOTRUMP West",
        pbn: "N:K75.KQ64.QJ75.65 8643.J8.AT64.T84 AT9.A7532.K98.K7 QJ2.T9.32.AQJ932",
        trump: NOTRUMP,
        leader: WEST,
        expected_ns_tricks: 6,
    },
    TestCase {
        name: "deal.24 SPADE West",
        pbn: "N:K75.KQ64.QJ75.65 8643.J8.AT64.T84 AT9.A7532.K98.K7 QJ2.T9.32.AQJ932",
        trump: SPADE,
        leader: WEST,
        expected_ns_tricks: 8,
    },
    TestCase {
        name: "deal.24 HEART West",
        pbn: "N:K75.KQ64.QJ75.65 8643.J8.AT64.T84 AT9.A7532.K98.K7 QJ2.T9.32.AQJ932",
        trump: HEART,
        leader: WEST,
        expected_ns_tricks: 10,
    },
    TestCase {
        name: "deal.24 DIAMOND West",
        pbn: "N:K75.KQ64.QJ75.65 8643.J8.AT64.T84 AT9.A7532.K98.K7 QJ2.T9.32.AQJ932",
        trump: DIAMOND,
        leader: WEST,
        expected_ns_tricks: 9,
    },
    TestCase {
        name: "deal.24 CLUB West",
        pbn: "N:K75.KQ64.QJ75.65 8643.J8.AT64.T84 AT9.A7532.K98.K7 QJ2.T9.32.AQJ932",
        trump: CLUB,
        leader: WEST,
        expected_ns_tricks: 5,
    },
    TestCase {
        name: "deal.25 NOTRUMP West",
        pbn: "N:J643.J3.JT5.Q652 A72.Q864.Q73.KT3 Q985.92.K2.AJ874 KT.AKT75.A9864.9",
        trump: NOTRUMP,
        leader: WEST,
        expected_ns_tricks: 2,
    },
    TestCase {
        name: "deal.25 SPADE West",
        pbn: "N:J643.J3.JT5.Q652 A72.Q864.Q73.KT3 Q985.92.K2.AJ874 KT.AKT75.A9864.9",
        trump: SPADE,
        leader: WEST,
        expected_ns_tricks: 7,
    },
    TestCase {
        name: "deal.25 HEART West",
        pbn: "N:J643.J3.JT5.Q652 A72.Q864.Q73.KT3 Q985.92.K2.AJ874 KT.AKT75.A9864.9",
        trump: HEART,
        leader: WEST,
        expected_ns_tricks: 2,
    },
    TestCase {
        name: "deal.25 DIAMOND West",
        pbn: "N:J643.J3.JT5.Q652 A72.Q864.Q73.KT3 Q985.92.K2.AJ874 KT.AKT75.A9864.9",
        trump: DIAMOND,
        leader: WEST,
        expected_ns_tricks: 2,
    },
    TestCase {
        name: "deal.25 CLUB West",
        pbn: "N:J643.J3.JT5.Q652 A72.Q864.Q73.KT3 Q985.92.K2.AJ874 KT.AKT75.A9864.9",
        trump: CLUB,
        leader: WEST,
        expected_ns_tricks: 6,
    },
];

#[test]
#[ignore] // Slow: runs 100+ DDS solver cases
fn test_all_cases() {
    for case in TEST_CASES {
        let hands = Hands::from_pbn(case.pbn)
            .unwrap_or_else(|| panic!("Failed to parse PBN for {}", case.name));

        let solver = Solver::new(hands, case.trump, case.leader);
        let ns_tricks = solver.solve();

        assert_eq!(
            ns_tricks, case.expected_ns_tricks,
            "{}: expected {} tricks, got {}",
            case.name, case.expected_ns_tricks, ns_tricks
        );
    }
}

#[test]
#[ignore] // Slow: runs DDS solver on upstream test cases
fn test_upstream_fixed_deals() {
    for case in UPSTREAM_TEST_CASES {
        let hands = Hands::from_pbn(case.pbn)
            .unwrap_or_else(|| panic!("Failed to parse PBN for {}", case.name));

        let solver = Solver::new(hands, case.trump, case.leader);
        let ns_tricks = solver.solve();

        assert_eq!(
            ns_tricks, case.expected_ns_tricks,
            "{}: expected {} tricks, got {}",
            case.name, case.expected_ns_tricks, ns_tricks
        );
    }
}

#[test]
fn test_cards_basic_operations() {
    let mut cards = Cards::new();
    assert!(cards.is_empty());

    cards.add(cards::card_of(SPADE, types::ACE));
    assert_eq!(cards.size(), 1);
    assert!(cards.have(cards::card_of(SPADE, types::ACE)));

    cards.add(cards::card_of(HEART, types::KING));
    assert_eq!(cards.size(), 2);

    let spades = cards.suit(SPADE);
    assert_eq!(spades.size(), 1);
}

#[test]
fn test_hands_parsing() {
    let pbn = "N:AKQT3.J6.KJ42.95 652.AK42.AQ87.T4 J74.QT95.T.AK863 98.873.9653.QJ72";
    let hands = Hands::from_pbn(pbn).unwrap();

    assert_eq!(hands[NORTH].size(), 13);
    assert_eq!(hands[EAST].size(), 13);
    assert_eq!(hands[SOUTH].size(), 13);
    assert_eq!(hands[WEST].size(), 13);
    assert_eq!(hands.all_cards().size(), 52);
}

/// Concurrent solves must not interfere with one another.
///
/// `dealer3` drives this library from a rayon pool, one deal per worker, so
/// several solves are always in flight at once. Nothing in the search is
/// shared between them — the caches are owned per call — and these tests exist
/// to keep it that way. Deliberately no timing assertion: scaling depends on
/// what else the machine is doing, and a speedup threshold would be flaky.
#[cfg(test)]
mod concurrency {
    use crate::{get_node_count, solve_dd_table};
    use bridge_types::Deal;

    const DEALS: &[&str] = &[
        "N:AQT94.T53.KQ8.T9 63.J98.J53.K8654 K72.6.AT9762.AJ7 J85.AKQ742.4.Q32",
        "E:7.AKJ6543.3.AK93 642.8.KQT9.T8752 AKQJ5.Q97.52.QJ6 T983.T2.AJ8764.4",
        "S:AJ83.7.T9653.A62 Q4.QJT984.4.T853 T762.K3.KQ2.KQJ4 K95.A652.AJ87.97",
        "W:87.9.T9642.AK942 AQJ53.QJ873.QJ5. 94.A652.K83.QJ75 KT62.KT4.A7.T863",
    ];

    fn deals() -> Vec<Deal> {
        DEALS
            .iter()
            .map(|s| Deal::from_pbn(s).expect("deal parses"))
            .collect()
    }

    fn solve_all_concurrently(deals: &[Deal]) -> Vec<[[u8; 5]; 4]> {
        std::thread::scope(|scope| {
            let handles: Vec<_> = deals
                .iter()
                .map(|deal| scope.spawn(move || solve_dd_table(deal).tricks))
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().expect("solver thread panicked"))
                .collect()
        })
    }

    #[test]
    fn concurrent_solves_agree_with_serial() {
        let deals = deals();
        let serial: Vec<_> = deals.iter().map(|d| solve_dd_table(d).tricks).collect();
        assert_eq!(serial, solve_all_concurrently(&deals));
    }

    #[test]
    fn concurrent_solves_are_deterministic() {
        let deals = deals();
        let first = solve_all_concurrently(&deals);
        for _ in 0..2 {
            assert_eq!(first, solve_all_concurrently(&deals));
        }
    }

    /// The node count reports this thread's own last solve. It used to be a
    /// process-wide atomic, which both made the number meaningless under
    /// concurrency and — being incremented once per node — serialised the
    /// search on one cache line.
    #[test]
    fn node_count_is_per_thread() {
        let deals = deals();
        let _ = solve_dd_table(&deals[0]);
        let ours = get_node_count();
        assert!(ours > 0, "no nodes recorded");

        std::thread::scope(|scope| {
            for deal in &deals[1..] {
                scope.spawn(move || {
                    let _ = solve_dd_table(deal);
                    assert!(get_node_count() > 0, "worker recorded no nodes");
                });
            }
        });

        assert_eq!(ours, get_node_count(), "another thread clobbered the count");
    }
}
