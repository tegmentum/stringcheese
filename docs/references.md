# Comparand — Bibliography

Every algorithm in the Comparand toolkit is derived from published work.
This file catalogs every paper, standard, and book the workspace cites,
in a single place, for auditing and further reading.

Entries are listed alphabetically by first author's surname, then by year,
within each section. Each entry links back to the Comparand crate(s) that
cite it.

Citation format:

> Author (Year). "Title." *Venue*, volume(issue), pages. DOI/URL.

DOIs are included only where they have been verified against publicly
available paper metadata. Where a specific field (issue number, page
range, DOI) could not be verified with confidence, it is omitted rather
than guessed. See [Citation status](#citation-status) at the bottom of
this file.

## Contents

- [Edit distance and alignment](#edit-distance-and-alignment)
- [String similarity](#string-similarity)
- [Set similarity and n-grams](#set-similarity-and-n-grams)
- [Phonetic matching](#phonetic-matching)
- [Substring search](#substring-search)
- [Content-defined chunking and rolling hashes](#content-defined-chunking-and-rolling-hashes)
- [Index structures](#index-structures)
- [Probabilistic sketches and LSH](#probabilistic-sketches-and-lsh)
- [Unicode standards](#unicode-standards)
- [General references](#general-references)
- [Citation status](#citation-status)

---

## Edit distance and alignment

### Bergroth, Hakonen, & Raita (2000)

Bergroth, L., Hakonen, H., & Raita, T. (2000). "A survey of longest
common subsequence algorithms." In *Proceedings Seventh International
Symposium on String Processing and Information Retrieval (SPIRE 2000)*,
pp. 39–48. IEEE.
DOI: https://doi.org/10.1109/SPIRE.2000.878178

Cited by: `comparand-lcs` (algorithm survey for LCS variants),
`comparand-levenshtein` (LCS relationship to unit-cost edit distance).

### Damerau (1964)

Damerau, F. J. (1964). "A technique for computer detection and correction
of spelling errors." *Communications of the ACM*, 7(3), 171–176.
DOI: https://doi.org/10.1145/363958.363994

Cited by: `comparand-damerau` (family DamerauLevenshtein; originates the
transposition-as-single-edit model).

### Gotoh (1982)

Gotoh, O. (1982). "An improved algorithm for matching biological
sequences." *Journal of Molecular Biology*, 162(3), 705–708.
DOI: https://doi.org/10.1016/0022-2836(82)90398-9

Cited by: `comparand-align` (affine gap penalties in Needleman-Wunsch and
Smith-Waterman).

### Hamming (1950)

Hamming, R. W. (1950). "Error detecting and error correcting codes." *Bell
System Technical Journal*, 29(2), 147–160.
DOI: https://doi.org/10.1002/j.1538-7305.1950.tb00463.x

Cited by: `comparand-hamming` (family Hamming; equal-length distance
counting mismatched positions).

### Henikoff & Henikoff (1992)

Henikoff, S., & Henikoff, J. G. (1992). "Amino acid substitution matrices
from protein blocks." *Proceedings of the National Academy of Sciences*,
89(22), 10915–10919.
DOI: https://doi.org/10.1073/pnas.89.22.10915

Cited by: `comparand-align` (BLOSUM substitution matrices as a canonical
scoring option for biological alignment).

### Hirschberg (1975)

Hirschberg, D. S. (1975). "A linear space algorithm for computing maximal
common subsequences." *Communications of the ACM*, 18(6), 341–343.
DOI: https://doi.org/10.1145/360825.360861

Cited by: `comparand-lcs` (O(n) space LCS reconstruction),
`comparand-align` (linear-space divide-and-conquer alignment recovery).

### Levenshtein (1966)

Levenshtein, V. I. (1966). "Binary codes capable of correcting deletions,
insertions, and reversals." *Soviet Physics Doklady*, 10(8), 707–710.
(English translation of *Doklady Akademii Nauk SSSR*, 163(4), 845–848,
1965.)

Cited by: `comparand-levenshtein` (family Levenshtein; originates the
insert/delete/substitute edit distance).

### Lowrance & Wagner (1975)

Lowrance, R., & Wagner, R. A. (1975). "An extension of the string-to-string
correction problem." *Journal of the ACM*, 22(2), 177–183.
DOI: https://doi.org/10.1145/321879.321880

Cited by: `comparand-damerau` (formal definition of unrestricted
Damerau-Levenshtein distance with adjacent transpositions).

### Needleman & Wunsch (1970)

Needleman, S. B., & Wunsch, C. D. (1970). "A general method applicable to
the search for similarities in the amino acid sequence of two proteins."
*Journal of Molecular Biology*, 48(3), 443–453.
DOI: https://doi.org/10.1016/0022-2836(70)90057-4

Cited by: `comparand-align` (global alignment; foundational
dynamic-programming formulation).

### Smith & Waterman (1981)

Smith, T. F., & Waterman, M. S. (1981). "Identification of common molecular
subsequences." *Journal of Molecular Biology*, 147(1), 195–197.
DOI: https://doi.org/10.1016/0022-2836(81)90087-5

Cited by: `comparand-align` (local alignment; scoring-based subsequence
identification).

### Ukkonen (1985)

Ukkonen, E. (1985). "Algorithms for approximate string matching."
*Information and Control*, 64(1–3), 100–118.
DOI: https://doi.org/10.1016/S0019-9958(85)80046-2

Cited by: `comparand-levenshtein` (banded / cutoff-aware edit-distance
computation and the diagonal-transition technique).

### Wagner & Fischer (1974)

Wagner, R. A., & Fischer, M. J. (1974). "The string-to-string correction
problem." *Journal of the ACM*, 21(1), 168–173.
DOI: https://doi.org/10.1145/321796.321811

Cited by: `comparand-levenshtein` (canonical dynamic-programming
formulation of edit distance), `comparand-lcs` (dual formulation for
longest common subsequence).

---

## String similarity

### Jaro (1989)

Jaro, M. A. (1989). "Advances in record-linkage methodology as applied to
matching the 1985 census of Tampa, Florida." *Journal of the American
Statistical Association*, 84(406), 414–420.
DOI: https://doi.org/10.1080/01621459.1989.10478785

Cited by: `comparand-jaro` (family Jaro; matching-window definition and
transposition count).

### Winkler (1990)

Winkler, W. E. (1990). "String comparator metrics and enhanced decision
rules in the Fellegi-Sunter model of record linkage." In *Proceedings of
the Section on Survey Research Methods*, American Statistical Association,
pp. 354–359. (Also published as U.S. Bureau of the Census research
report.)

Cited by: `comparand-jaro` (family JaroWinkler; prefix boost and
adjustable prefix scale).

### Winkler (1999)

Winkler, W. E. (1999). "The state of record linkage and current research
problems." *Statistical Research Division, U.S. Bureau of the Census*,
research report RR99/04.
URL: https://www.census.gov/library/working-papers/1999/adrm/rr99-04.html

Cited by: `comparand-jaro` (family JaroWinkler; the threshold-gated
variant `philips-1999-full` derives from Winkler's threshold refinement
introduced in this report). Also the source cited by
`JaroWinkler::WITH_THRESHOLD_DESCRIPTOR`.

---

## Set similarity and n-grams

### Broder (1997)

Broder, A. Z. (1997). "On the resemblance and containment of documents."
In *Proceedings of the Compression and Complexity of Sequences 1997*,
pp. 21–29. IEEE.
DOI: https://doi.org/10.1109/SEQUEN.1997.666900

Cited by: `comparand-set-similarity` (resemblance defined as Jaccard on
shingle sets, containment as an asymmetric variant),
`comparand-minhash` (MinHash sketch as an unbiased Jaccard estimator).

### Dice (1945)

Dice, L. R. (1945). "Measures of the amount of ecologic association
between species." *Ecology*, 26(3), 297–302.
DOI: https://doi.org/10.2307/1932409

Cited by: `comparand-set-similarity` (Dice / Sørensen–Dice coefficient).

### Jaccard (1912)

Jaccard, P. (1912). "The distribution of the flora in the alpine zone."
*New Phytologist*, 11(2), 37–50.
DOI: https://doi.org/10.1111/j.1469-8137.1912.tb05611.x

Cited by: `comparand-set-similarity` (Jaccard index over sets and
multisets).

### Manning, Raghavan, & Schütze (2008)

Manning, C. D., Raghavan, P., & Schütze, H. (2008). *Introduction to
Information Retrieval*. Cambridge University Press. ISBN 978-0-521-86571-5.

Cited by: `comparand-ngram` (n-gram indexing, cosine and Jaccard over
sparse term-frequency vectors), `comparand-set-similarity` (vector-space
similarity measures on shingle sets).

### Salton & McGill (1983)

Salton, G., & McGill, M. J. (1983). *Introduction to Modern Information
Retrieval*. McGraw-Hill. ISBN 978-0-07-054484-0.

Cited by: `comparand-set-similarity` (cosine similarity on tf–idf
weighted vectors), `comparand-ngram` (bag-of-n-grams representation and
weighting schemes).

### Simpson (1943)

Simpson, G. G. (1943). "Mammals and the nature of continents." *American
Journal of Science*, 241(1), 1–31.
DOI: https://doi.org/10.2475/ajs.241.1.1

Cited by: `comparand-set-similarity` (overlap / Simpson coefficient,
defined as intersection size divided by the smaller set size).

### Sørensen (1948)

Sørensen, T. (1948). "A method of establishing groups of equal amplitude
in plant sociology based on similarity of species content and its
application to analyses of the vegetation on Danish commons." *Kongelige
Danske Videnskabernes Selskab, Biologiske Skrifter*, 5(4), 1–34.

Cited by: `comparand-set-similarity` (Sørensen–Dice coefficient; the
biological-ecology origin of the same formula independently proposed by
Dice).

### Szymkiewicz (1934)

Szymkiewicz, D. (1934). "Une contribution statistique à la géographie
floristique." *Acta Societatis Botanicorum Poloniae*, 11(3), 249–265.

Cited by: `comparand-set-similarity` (early formulation of the overlap
coefficient; often co-cited with Simpson (1943)).

### Ukkonen (1992)

Ukkonen, E. (1992). "Approximate string-matching with q-grams and maximal
matches." *Theoretical Computer Science*, 92(1), 191–211.
DOI: https://doi.org/10.1016/0304-3975(92)90143-4

Cited by: `comparand-ngram` (q-gram profile distance and the q-gram
lower bound on edit distance), `comparand-index` (q-gram count filter
used to prune candidate pairs in edit-distance joins).

---

## Phonetic matching

### NARA Soundex (undated)

U.S. National Archives and Records Administration. "The Soundex Indexing
System." Reference specification, National Archives.
URL: https://www.archives.gov/research/census/soundex

Cited by: `comparand-phonetic` (the widely-implemented NARA-normative
Soundex rules; used as the reference specification against which the
`Soundex` variant is validated).

### Odell & Russell (1918)

Odell, M. K., & Russell, R. C. (1918). "Soundex system." U.S. Patent
No. 1,261,167. Filed October 25, 1917; issued April 2, 1918.
URL: https://patents.google.com/patent/US1261167

Cited by: `comparand-phonetic` (original Soundex patent; historical
source of the family SoundexOdellRussell variant).

### Philips (1990)

Philips, L. (1990). "Hanging on the Metaphone." *Computer Language
Magazine*, 7(12), 39–43.

Cited by: `comparand-phonetic` (family Metaphone; original 16-code
English phonetic algorithm).

### Philips (2000)

Philips, L. (2000). "The Double Metaphone search algorithm." *C/C++
Users Journal*, 18(6), 38–43.

Cited by: `comparand-phonetic` (family DoubleMetaphone; primary and
alternate keys for names of non-English origin).

### Taft (1970)

Taft, R. L. (1970). "Name search techniques." *Special Report No. 1*, New
York State Identification and Intelligence System (NYSIIS), Bureau of
Systems Development, Albany, NY.

Cited by: `comparand-phonetic` (family NYSIIS; rule set for the New
York State Identification and Intelligence System phonetic key).

---

## Substring search

### Aho & Corasick (1975)

Aho, A. V., & Corasick, M. J. (1975). "Efficient string matching: an aid
to bibliographic search." *Communications of the ACM*, 18(6), 333–340.
DOI: https://doi.org/10.1145/360825.360855

Cited by: `comparand-search` (multi-pattern Aho-Corasick automaton with
failure links).

### Boyer & Moore (1977)

Boyer, R. S., & Moore, J S. (1977). "A fast string searching algorithm."
*Communications of the ACM*, 20(10), 762–772.
DOI: https://doi.org/10.1145/359842.359859

Cited by: `comparand-search` (Boyer-Moore search with bad-character and
good-suffix rules).

### Charras & Lecroq (2004)

Charras, C., & Lecroq, T. (2004). *Handbook of Exact String Matching
Algorithms*. King's College Publications. ISBN 978-0-9543006-4-9.
Companion site: http://www-igm.univ-mlv.fr/~lecroq/string/

Cited by: `comparand-search` (reference survey used to cross-check
pseudocode, edge cases, and shift-table constructions for every
single-pattern algorithm in the crate).

### Crochemore & Perrin (1991)

Crochemore, M., & Perrin, D. (1991). "Two-way string-matching." *Journal
of the ACM*, 38(3), 650–674.
DOI: https://doi.org/10.1145/116825.116845

Cited by: `comparand-search` (two-way string matching; the algorithm
underlying Rust's `str::find` and many modern libc `memmem`
implementations).

### Crochemore, Hancart, & Lecroq (2007)

Crochemore, M., Hancart, C., & Lecroq, T. (2007). *Algorithms on
Strings*. Cambridge University Press. ISBN 978-0-521-84899-2.

Cited by: `comparand-search` (comprehensive treatment of two-way string
matching and its complexity analysis; used to cross-check the critical-
factorization derivation).

### Galil (1979)

Galil, Z. (1979). "On improving the worst case running time of the
Boyer-Moore string matching algorithm." *Communications of the ACM*,
22(9), 505–508.
DOI: https://doi.org/10.1145/359146.359148

Cited by: `comparand-search` (Galil's rule; supplies the linear-time
worst-case bound for Boyer-Moore).

### Horspool (1980)

Horspool, R. N. (1980). "Practical fast searching in strings."
*Software: Practice and Experience*, 10(6), 501–506.
DOI: https://doi.org/10.1002/spe.4380100608

Cited by: `comparand-search` (Boyer-Moore-Horspool; simplified
bad-character-only variant).

### Karp & Rabin (1987)

Karp, R. M., & Rabin, M. O. (1987). "Efficient randomized pattern-matching
algorithms." *IBM Journal of Research and Development*, 31(2), 249–260.
DOI: https://doi.org/10.1147/rd.312.0249

Cited by: `comparand-search` (Rabin-Karp rolling-hash pattern search),
`comparand-cdc` (polynomial rolling-hash construction reused by content
chunkers).

### Knuth, Morris, & Pratt (1977)

Knuth, D. E., Morris, J. H., & Pratt, V. R. (1977). "Fast pattern matching
in strings." *SIAM Journal on Computing*, 6(2), 323–350.
DOI: https://doi.org/10.1137/0206024

Cited by: `comparand-search` (KMP failure function and linear-time
single-pattern search).

---

## Content-defined chunking and rolling hashes

### Broder (1993)

Broder, A. Z. (1993). "Some applications of Rabin's fingerprinting
method." In R. Capocelli, A. De Santis, & U. Vaccaro (Eds.), *Sequences II:
Methods in Communication, Security, and Computer Science*, pp. 143–152.
Springer-Verlag.
DOI: https://doi.org/10.1007/978-1-4613-9323-8_11

Cited by: `comparand-cdc` (rolling / sliding-window Rabin fingerprints
as the classical basis for content-defined chunking).

### Muthitacharoen, Chen, & Mazières (2001)

Muthitacharoen, A., Chen, B., & Mazières, D. (2001). "A low-bandwidth
network file system." In *Proceedings of the Eighteenth ACM Symposium on
Operating Systems Principles (SOSP '01)*, pp. 174–187.
DOI: https://doi.org/10.1145/502034.502052

Cited by: `comparand-cdc` (LBFS; introduced Rabin-fingerprint
content-defined chunking with min/max chunk-size constraints for
deduplication).

### Rabin (1981)

Rabin, M. O. (1981). "Fingerprinting by random polynomials." *Technical
Report TR-15-81*, Center for Research in Computing Technology, Harvard
University.

Cited by: `comparand-cdc` (Rabin polynomial fingerprint; the foundation
for rolling-hash CDC schemes).

### Xia, Jiang, Feng, Hua, Hu, Liu, & Zhang (2016)

Xia, W., Jiang, H., Feng, D., Hua, Y., Hu, Y., Liu, Q., & Zhang, Y.
(2016). "FastCDC: A fast and efficient content-defined chunking approach
for data deduplication." In *2016 USENIX Annual Technical Conference
(USENIX ATC '16)*, pp. 101–114.
URL: https://www.usenix.org/conference/atc16/technical-sessions/presentation/xia

Cited by: `comparand-cdc` (FastCDC; Gear hash, normalized chunking,
sub-minimum cut-point skipping).

---

## Index structures

### Baeza-Yates & Ribeiro-Neto (2011)

Baeza-Yates, R., & Ribeiro-Neto, B. (2011). *Modern Information
Retrieval: The Concepts and Technology Behind Search* (2nd ed.).
Addison-Wesley Professional. ISBN 978-0-321-41691-9.

Cited by: `comparand-index` (modern reference for metric-space
indexing, cited in the BK-tree module as background for tree-based
best-match search).

### Burkhard & Keller (1973)

Burkhard, W. A., & Keller, R. M. (1973). "Some approaches to best-match
file searching." *Communications of the ACM*, 16(4), 230–236.
DOI: https://doi.org/10.1145/362003.362025

Cited by: `comparand-index` (BK-tree; metric tree keyed on discrete
integer distances, requires a true metric).

### Christen (2012)

Christen, P. (2012). *Data Matching: Concepts and Techniques for Record
Linkage, Entity Resolution, and Duplicate Detection*. Data-Centric Systems
and Applications. Springer. ISBN 978-3-642-31163-5.
DOI: https://doi.org/10.1007/978-3-642-31164-2

Cited by: `comparand-set-similarity` (overview of similarity measures
in the entity-resolution context).

### Sarawagi & Kirpal (2004)

Sarawagi, S., & Kirpal, A. (2004). "Efficient set joins on similarity
predicates." In *Proceedings of the 2004 ACM SIGMOD International
Conference on Management of Data*, pp. 743–754.
DOI: https://doi.org/10.1145/1007568.1007652

Cited by: `comparand-index` (set-join length filter and prefix-filter
techniques for pruning candidate pairs under Jaccard / cosine thresholds).

### Yianilos (1993)

Yianilos, P. N. (1993). "Data structures and algorithms for nearest
neighbor search in general metric spaces." In *Proceedings of the Fourth
Annual ACM-SIAM Symposium on Discrete Algorithms (SODA '93)*, pp. 311–321.

Cited by: `comparand-index` (VP-tree; vantage-point partitioning for
nearest-neighbor search in a general metric space).

---

## Probabilistic sketches and LSH

### Broder (1997)

See [Broder (1997)](#broder-1997) under Set similarity and n-grams.

Also cited by: `comparand-minhash` (originates the MinHash sketch as
an unbiased estimator of Jaccard resemblance, via min-wise independent
permutations).

### Fowler, Noll, & Vo (undated)

Fowler, G., Noll, L. C., & Vo, K.-P. "FNV Non-Cryptographic Hash
Algorithm." Reference specification.
URL: http://www.isthe.com/chongo/tech/comp/fnv/

Cited by: `comparand-minhash` (FNV-1a as a fast, well-distributed
non-cryptographic hash used to seed sketch families and mix shingles).

### Gionis, Indyk, & Motwani (1999)

Gionis, A., Indyk, P., & Motwani, R. (1999). "Similarity search in high
dimensions via hashing." In *Proceedings of the 25th International
Conference on Very Large Data Bases (VLDB '99)*, pp. 518–529. Morgan
Kaufmann.

Cited by: `comparand-minhash` (canonical formulation of
locality-sensitive hashing: banded LSH over MinHash signatures for
approximate near-neighbor search).

### Ioffe (2010)

Ioffe, S. (2010). "Improved consistent sampling, weighted MinHash and L1
sketching." In *Proceedings of the 2010 IEEE International Conference on
Data Mining (ICDM '10)*, pp. 246–255. IEEE.
DOI: https://doi.org/10.1109/ICDM.2010.80

Cited by: `comparand-minhash` (consistent weighted sampling; weighted
MinHash for real-valued Jaccard and L1 similarity).

### Leskovec, Rajaraman, & Ullman (2020)

Leskovec, J., Rajaraman, A., & Ullman, J. D. (2020). *Mining of Massive
Datasets* (3rd ed.). Cambridge University Press. ISBN 978-1-108-47634-8.
Companion site: http://www.mmds.org/

Cited by: `comparand-minhash` (Chapter 3 as a self-contained treatment
of shingling, MinHash, LSH banding, and threshold-to-band-count tuning).

### Shrivastava (2014)

Shrivastava, A. (2014). "Optimal densification for fast and accurate
minwise hashing." arXiv:1406.4784.
URL: https://arxiv.org/abs/1406.4784

Cited by: `comparand-minhash` (referenced in prose in the sketch module
as the standard densification technique for the one-permutation MinHash
alternative to this crate's k-permutation implementation; the crate
carries the k-permutation approach because its unbiasedness proof does
not require densification).

### Steele, Lea, & Flood (2014)

Steele, G. L., Lea, D., & Flood, C. H. (2014). "Fast splittable
pseudorandom number generators." In *Proceedings of the 2014 ACM
International Conference on Object-Oriented Programming Systems Languages
& Applications (OOPSLA '14)*, pp. 453–472.
DOI: https://doi.org/10.1145/2660193.2660195

Cited by: `comparand-minhash` (SplitMix64 finalizer used as a fast,
high-avalanche mixing function inside MinHash permutation families).

---

## Unicode standards

### Unicode Consortium (2022) — Unicode Standard 15.0.0

The Unicode Consortium. (2022). *The Unicode Standard, Version 15.0.0*.
Mountain View, CA: The Unicode Consortium. ISBN 978-1-936213-32-0.
URL: https://www.unicode.org/versions/Unicode15.0.0/

Cited by: `comparand-unicode` (base version of the Unicode Character
Database consumed by normalization, case folding, and grapheme
segmentation tables), `comparand-core` (Unicode-scalar-value and
grapheme-cluster sequence models).

### Unicode Standard Annex #15 — Normalization Forms

Davis, M., & Whistler, K. (Eds.). *Unicode Standard Annex #15: Unicode
Normalization Forms*. The Unicode Consortium.
URL: https://www.unicode.org/reports/tr15/

Cited by: `comparand-unicode` (NFC, NFD, NFKC, NFKD normalization
pipelines).

### Unicode Standard Annex #21 — Case Mappings

*Unicode Standard Annex #21: Case Mappings*. The Unicode Consortium.
URL: https://www.unicode.org/reports/tr21/

Cited by: `comparand-unicode` (locale-sensitive and locale-independent
case-folding rules; interaction with normalization).

### Unicode Standard Annex #29 — Text Segmentation

Davis, M. (Ed.). *Unicode Standard Annex #29: Unicode Text Segmentation*.
The Unicode Consortium.
URL: https://www.unicode.org/reports/tr29/

Cited by: `comparand-unicode` (grapheme cluster, word, and sentence
segmentation boundaries), `comparand-core` (grapheme-boundary iteration
underlying the grapheme sequence representation).

### CaseFolding.txt — Unicode Character Database

The Unicode Consortium. *CaseFolding.txt* (Unicode Character Database
data file).
URL: https://www.unicode.org/Public/UCD/latest/ucd/CaseFolding.txt

Cited by: `comparand-unicode` (canonical full case-folding table used
by the case-insensitive preprocessing stage; C (common) + F (full)
mappings).

---

## General references

### Cormen, Leiserson, Rivest, & Stein (2009)

Cormen, T. H., Leiserson, C. E., Rivest, R. L., & Stein, C. (2009).
*Introduction to Algorithms* (3rd ed.). MIT Press. ISBN 978-0-262-03384-8.

Cited by: `comparand-lcs` (Chapter 15, *Dynamic Programming*, longest
common subsequence as the running example), `comparand-search`
(Chapter 32, *String Matching*, textbook exposition of naive, KMP, and
Rabin-Karp), `comparand-core` (general algorithm-analysis conventions
for cost documentation).

---

## Citation status

This bibliography is a best-effort catalog. Entries have been verified
against publicly-available paper metadata (author lists, journal
volumes, DOIs where available). Some pre-2000 papers may have
incomplete DOIs or venue details; where a specific field is uncertain,
it is either omitted or marked "unverified" rather than guessed.

Specific caveats:

- **Levenshtein (1966)** has no DOI; the citation records the English
  translation venue as published in *Soviet Physics Doklady*.
- **Sørensen (1948)** and **Szymkiewicz (1934)** predate DOI assignment
  for their venues and are cited by journal and year only.
- **Winkler (1990)**, **Taft (1970)**, **Philips (1990)**, and
  **Philips (2000)** were published in venues (ASA proceedings, an
  agency technical report, and trade magazines) that do not have DOIs;
  they are cited by their original venue and page range where known.
- **Rabin (1981)** is a Harvard technical report and has no DOI; it is
  cited by report number.
- **Yianilos (1993)** and **Gionis, Indyk, & Motwani (1999)** are
  conference papers from SODA and VLDB respectively that predate
  systematic DOI assignment in their proceedings; DOIs are omitted.
- **NARA Soundex** and the **Unicode Standard Annexes** are living
  reference specifications rather than dated publications; they are
  cited by URL to the current authoritative version.

Corrections and additions are welcome via pull request.
