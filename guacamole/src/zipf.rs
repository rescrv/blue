use rand::Rng;

use super::Guacamole;

const ZIPFS: &[Zipf] = &[
    Zipf { n: 10000000, alpha: 1.1111111111111112, theta: 0.1, zetan: 2.216_957_624_468_751e6, zeta2: 0.0, eta: 0.9999990647515522},
    Zipf { n: 10000000, alpha: 1.25, theta: 0.2, zetan: 497633.2491763715, zeta2: 0.0, eta: 0.9999956265517043},
    Zipf { n: 10000000, alpha: 1.4285714285714286, theta: 0.3, zetan: 113474.56151585997, zeta2: 0.0, eta: 0.9999795486963488},
    Zipf { n: 10000000, alpha: 1.6666666666666667, theta: 0.4, zetan: 26413.75253568109, zeta2: 0.0, eta: 0.999904364750021},
    Zipf { n: 10000000, alpha: 2.0, theta: 0.5, zetan: 6323.095123940201, zeta2: 0.0, eta: 0.9995527864045001},
    Zipf { n: 10000000, alpha: 2.5, theta: 0.6, zetan: 1575.4407313003119, zeta2: 0.0, eta: 0.9979087208948174},
    Zipf { n: 10000000, alpha: 3.333333333333333, theta: 0.7, zetan: 416.863421780444, zeta2: 0.0, eta: 0.9902206723145707},
    Zipf { n: 10000000, alpha: 5.000000000000001, theta: 0.8, zetan: 121.15678441550145, zeta2: 0.0, eta: 0.9542694948072673},
    Zipf { n: 10000000, alpha: 10.000000000000002, theta: 0.9, zetan: 40.68860959391765, zeta2: 0.0, eta: 0.7861530800017623},
    Zipf { n: 10000000, alpha: 1.0, theta: 0.0, zetan: 1e7, zeta2: 0.0, eta: 0.9999998},
    Zipf { n: 10000000, alpha: 10.0, theta: 0.9, zetan: 40.68860959391765, zeta2: 0.0, eta: 0.7861530800017623},
    Zipf { n: 10000000, alpha: 100.0, theta: 0.99, zetan: 18.066242574968303, zeta2: 0.0, eta: 0.14294182856772952},
    Zipf { n: 10000000, alpha: 1000.0, theta: 0.999, zetan: 16.825835765719074, zeta2: 0.0, eta: 0.015306593275091429},
    Zipf { n: 10000000, alpha: 10000.0, theta: 0.9999, zetan: 16.70830071644092, zeta2: 0.0, eta: 0.0015413058133011415},
    Zipf { n: 100000000, alpha: 1.1111111111111112, theta: 0.1, zetan: 1.760_992_383_688_752_7e7, zeta2: 0.0, eta: 0.9999998822591962},
    Zipf { n: 100000000, alpha: 1.25, theta: 0.2, zetan: 3.139_857_318_025_828e6, zeta2: 0.0, eta: 0.9999993068551568},
    Zipf { n: 100000000, alpha: 1.4285714285714286, theta: 0.3, zetan: 568723.6267934917, zeta2: 0.0, eta: 0.9999959194284532},
    Zipf { n: 100000000, alpha: 1.6666666666666667, theta: 0.4, zetan: 105158.42293108889, zeta2: 0.0, eta: 0.9999759775113204},
    Zipf { n: 100000000, alpha: 2.0, theta: 0.5, zetan: 19998.53969549304, zeta2: 0.0, eta: 0.9998585786437627},
    Zipf { n: 100000000, alpha: 2.5, theta: 0.6, zetan: 3960.280327627551, zeta2: 0.0, eta: 0.9991674467925982},
    Zipf { n: 100000000, alpha: 3.333333333333333, theta: 0.7, zetan: 834.5170899801662, zeta2: 0.0, eta: 0.9950987258106051},
    Zipf { n: 100000000, alpha: 5.000000000000001, theta: 0.8, zetan: 194.6160470599037, zeta2: 0.0, eta: 0.9711460018818557},
    Zipf { n: 100000000, alpha: 10.000000000000002, theta: 0.9, zetan: 53.665620460177784, zeta2: 0.0, eta: 0.8301353535365752},
    Zipf { n: 100000000, alpha: 1.0, theta: 0.0, zetan: 1e8, zeta2: 0.0, eta: 0.99999998},
    Zipf { n: 100000000, alpha: 10.0, theta: 0.9, zetan: 53.665620460177784, zeta2: 0.0, eta: 0.8301353535365752},
    Zipf { n: 100000000, alpha: 100.0, theta: 0.99, zetan: 20.80293049002014, zeta2: 0.0, eta: 0.1624508543520593},
    Zipf { n: 100000000, alpha: 1000.0, theta: 0.999, zetan: 19.168530903421356, zeta2: 0.0, eta: 0.01757132526491123},
    Zipf { n: 100000000, alpha: 10000.0, theta: 0.9999, zetan: 19.01486562854262, zeta2: 0.0, eta: 0.001771182957221673},
    Zipf { n: 1000000000, alpha: 1.1111111111111112, theta: 0.1, zetan: 1.398_806_007_703_687_3e8, zeta2: 0.0, eta: 0.999999985177311},
    Zipf { n: 1000000000, alpha: 1.25, theta: 0.2, zetan: 1.981_116_417_976_812_3e7, zeta2: 0.0, eta: 0.9999998901439456},
    Zipf { n: 1000000000, alpha: 1.4285714285714286, theta: 0.3, zetan: 2.850_373_832_108_431e6, zeta2: 0.0, eta: 0.9999991858189369},
    Zipf { n: 1000000000, alpha: 1.6666666666666667, theta: 0.4, zetan: 418646.6039127252, zeta2: 0.0, eta: 0.9999939658236634},
    Zipf { n: 1000000000, alpha: 2.0, theta: 0.5, zetan: 63244.092864672115, zeta2: 0.0, eta: 0.99995527864045},
    Zipf { n: 1000000000, alpha: 2.5, theta: 0.6, zetan: 9950.726604378511, zeta2: 0.0, eta: 0.999668554598266},
    Zipf { n: 1000000000, alpha: 3.333333333333333, theta: 0.7, zetan: 1667.8457238955966, zeta2: 0.0, eta: 0.9975435439477685},
    Zipf { n: 1000000000, alpha: 5.000000000000001, theta: 0.8, zetan: 311.0411338557673, zeta2: 0.0, eta: 0.9817943579697391},
    Zipf { n: 1000000000, alpha: 10.000000000000002, theta: 0.9, zetan: 70.00270945700423, zeta2: 0.0, eta: 0.8650717152326436},
    Zipf { n: 1000000000, alpha: 1.0, theta: 0.0, zetan: 1e9, zeta2: 0.0, eta: 0.999999998},
    Zipf { n: 1000000000, alpha: 10.0, theta: 0.9, zetan: 70.00270945700423, zeta2: 0.0, eta: 0.8650717152326436},
    Zipf { n: 1000000000, alpha: 100.0, theta: 0.99, zetan: 23.60336410411734, zeta2: 0.0, eta: 0.1815158004930929},
    Zipf { n: 1000000000, alpha: 1000.0, theta: 0.999, zetan: 21.516626552401423, zeta2: 0.0, eta: 0.0198308485156955},
    Zipf { n: 1000000000, alpha: 10000.0, theta: 0.9999, zetan: 21.321961748562106, zeta2: 0.0, eta: 0.0020010071760672155},
    Zipf { n: 10000000000, alpha: 1.1111111111111112, theta: 0.1, zetan: 1.111_111_110_561_936_6e9, zeta2: 0.0, eta: 0.999999998133934},
    Zipf { n: 10000000000, alpha: 1.25, theta: 0.2, zetan: 1.249_999_992_710_837_9e8, zeta2: 0.0, eta: 0.9999999825889887},
    Zipf { n: 10000000000, alpha: 1.4285714285714286, theta: 0.3, zetan: 1.428_571_338_165_236_5e7, zeta2: 0.0, eta: 0.9999998375495207},
    Zipf { n: 10000000000, alpha: 1.6666666666666667, theta: 0.4, zetan: 1.666_665_531_918_927_3e6, zeta2: 0.0, eta: 0.9999984842834335},
    Zipf { n: 10000000000, alpha: 2.0, theta: 0.5, zetan: 199998.53965056606, zeta2: 0.0, eta: 0.9999858578643762},
    Zipf { n: 10000000000, alpha: 2.5, theta: 0.6, zetan: 24998.047339044355, zeta2: 0.0, eta: 0.9998680492089227},
    Zipf { n: 10000000000, alpha: 3.333333333333333, theta: 0.7, zetan: 3330.5549449376763, zeta2: 0.0, eta: 0.9987688555866551},
    Zipf { n: 10000000000, alpha: 5.000000000000001, theta: 0.8, zetan: 495.5624615889921, zeta2: 0.0, eta: 0.9885130164500296},
    Zipf { n: 10000000000, alpha: 10.000000000000002, theta: 0.9, zetan: 90.56988598108148, zeta2: 0.0, eta: 0.8928226537463706},
    Zipf { n: 10000000000, alpha: 1.0, theta: 0.0, zetan: 1e10, zeta2: 0.0, eta: 0.9999999998},
    Zipf { n: 10000000000, alpha: 10.0, theta: 0.9, zetan: 90.56988598108148, zeta2: 0.0, eta: 0.8928226537463706},
    Zipf { n: 10000000000, alpha: 100.0, theta: 0.99, zetan: 26.46902820178302, zeta2: 0.0, eta: 0.20014677547762882},
    Zipf { n: 10000000000, alpha: 1000.0, theta: 0.999, zetan: 23.870135124976315, zeta2: 0.0, eta: 0.02208517500721141},
    Zipf { n: 10000000000, alpha: 10000.0, theta: 0.9999, zetan: 23.62958916236007, zeta2: 0.0, eta: 0.0022307784820226884},
    Zipf { n: 100000000000, alpha: 1.1111111111111112, theta: 0.1, zetan: 8.825_869_276_134_706e9, zeta2: 0.0, eta: 0.9999999997650763},
    Zipf { n: 100000000000, alpha: 1.25, theta: 0.2, zetan: 7.886_966_799_353_771e8, zeta2: 0.0, eta: 0.9999999972405407},
    Zipf { n: 100000000000, alpha: 1.4285714285714286, theta: 0.3, zetan: 7.159_817_531_566_392e7, zeta2: 0.0, eta: 0.999999967586868},
    Zipf { n: 100000000000, alpha: 1.6666666666666667, theta: 0.4, zetan: 6.635_118_374_721_357e6, zeta2: 0.0, eta: 0.9999996192692122},
    Zipf { n: 100000000000, alpha: 2.0, theta: 0.5, zetan: 632454.0716856867, zeta2: 0.0, eta: 0.999995527864045},
    Zipf { n: 100000000000, alpha: 2.5, theta: 0.6, zetan: 62795.208125225625, zeta2: 0.0, eta: 0.9999474694439119},
    Zipf { n: 100000000000, alpha: 3.333333333333333, theta: 0.7, zetan: 6648.095994617613, zeta2: 0.0, eta: 0.99938296613728},
    Zipf { n: 100000000000, alpha: 5.000000000000001, theta: 0.8, zetan: 788.0090577810104, zeta2: 0.0, eta: 0.992752203363223},
    Zipf { n: 100000000000, alpha: 10.000000000000002, theta: 0.9, zetan: 116.46242715354546, zeta2: 0.0, eta: 0.9148660077479215},
    Zipf { n: 100000000000, alpha: 1.0, theta: 0.0, zetan: 1e11, zeta2: 0.0, eta: 0.99999999998},
    Zipf { n: 100000000000, alpha: 10.0, theta: 0.9, zetan: 116.46242715354546, zeta2: 0.0, eta: 0.9148660077479215},
    Zipf { n: 100000000000, alpha: 100.0, theta: 0.99, zetan: 29.40144218795688, zeta2: 0.0, eta: 0.21835365769521398},
    Zipf { n: 100000000000, alpha: 1000.0, theta: 0.999, zetan: 26.22906909062428, zeta2: 0.0, eta: 0.02433431669167374},
    Zipf { n: 100000000000, alpha: 10000.0, theta: 0.9999, zetan: 25.937747984051878, zeta2: 0.0, eta: 0.002460496887270569},
    Zipf { n: 1000000000000, alpha: 1.1111111111111112, theta: 0.1, zetan: 7.010_637_131_874_431e10, zeta2: 0.0, eta: 0.9999999999704249},
    Zipf { n: 1000000000000, alpha: 1.25, theta: 0.2, zetan: 4.976_339_733_841_656e9, zeta2: 0.0, eta: 0.9999999995626552},
    Zipf { n: 1000000000000, alpha: 1.4285714285714286, theta: 0.3, zetan: 3.588_409_147_950_952_6e8, zeta2: 0.0, eta: 0.99999999353273},
    Zipf { n: 1000000000000, alpha: 1.6666666666666667, theta: 0.4, zetan: 2.641_488_552_864_694_2e7, zeta2: 0.0, eta: 0.9999999043647501},
    Zipf { n: 1000000000000, alpha: 2.0, theta: 0.5, zetan: 1.999_998_535_690_452_7e6, zeta2: 0.0, eta: 0.9999985857864376},
    Zipf { n: 1000000000000, alpha: 2.5, theta: 0.6, zetan: 157737.38288988697, zeta2: 0.0, eta: 0.9999790872089481},
    Zipf { n: 1000000000000, alpha: 3.333333333333333, theta: 0.7, zetan: 13267.460663681399, zeta2: 0.0, eta: 0.999690750505289},
    Zipf { n: 1000000000000, alpha: 5.000000000000001, theta: 0.8, zetan: 1251.5056766938928, zeta2: 0.0, eta: 0.9954269494807267},
    Zipf { n: 1000000000000, alpha: 10.000000000000002, theta: 0.9, zetan: 149.05920676466877, zeta2: 0.0, eta: 0.9323756662193758},
    Zipf { n: 1000000000000, alpha: 1.0, theta: 0.0, zetan: 1e12, zeta2: 0.0, eta: 0.999999999998},
    Zipf { n: 1000000000000, alpha: 10.0, theta: 0.9, zetan: 149.05920676466877, zeta2: 0.0, eta: 0.9323756662193758},
    Zipf { n: 1000000000000, alpha: 100.0, theta: 0.99, zetan: 32.402164232015615, zeta2: 0.0, eta: 0.23614610067579656},
    Zipf { n: 1000000000000, alpha: 1000.0, theta: 0.999, zetan: 28.59344125820815, zeta2: 0.0, eta: 0.026578285493807807},
    Zipf { n: 1000000000000, alpha: 10000.0, theta: 0.9999, zetan: 28.246438229354407, zeta2: 0.0, eta: 0.002690162403990004},
];

/////////////////////////////////////////////// Zipf ///////////////////////////////////////////////

/// Zipf generator over [0, n).  From:
///
/// "Quickly Generating Billion-Record Synthetic Databases."
/// Gray et.al., SIGMOD 1994
///
/// This should be used *only* where it's OK to not be a perfect Zipf distribution.  For larger
/// distributions, low-rank items are missing and the curve is bent.  It's an approximation meant
/// to skew workloads for a key-value-store generator.
#[derive(Clone, Debug)]
pub struct Zipf {
    n: u64,
    alpha: f64,
    theta: f64,
    zetan: f64,
    zeta2: f64,
    eta: f64,
}

impl Zipf {
    /// Create a new Zipf distribution for `n` objects with `alpha > 1`.
    pub fn from_alpha(n: u64, alpha: f64) -> Self {
        for precomputed in ZIPFS.iter() {
            if precomputed.n == n && precomputed.alpha >= alpha * 0.999 && precomputed.alpha < alpha * 1.001 {
                return precomputed.clone();
            }
        }
        let mut zipf = Zipf {
            n,
            alpha,
            theta: 1.0 - 1.0 / alpha,
            zetan: 0.0,
            zeta2: 0.0,
            eta: 0.0,
        };
        zipf.init();
        zipf
    }

    /// Create a new Zipf distribution for `n` objects with `theta` over `(0, 1)`.
    pub fn from_theta(n: u64, theta: f64) -> Self {
        for precomputed in ZIPFS.iter() {
            if precomputed.n == n && precomputed.theta >= theta * 0.999 && precomputed.theta < theta * 1.001 {
                return precomputed.clone();
            }
        }
        let mut zipf = Zipf {
            n,
            theta,
            alpha: 1.0 / (1.0 - theta),
            zetan: 0.0,
            zeta2: 0.0,
            eta: 0.0,
        };
        zipf.init();
        zipf
    }

    /// Use `guac` to generate some randomness and then adjust so that the returned u64 obeys a
    /// Zipf distribution on [0, n), where 0 is the most common element, 1 the next most common and
    /// so on.  It's not perfect, so expect to see cases where the distribution doesn't hold.
    pub fn next(&self, guac: &mut Guacamole) -> u64 {
        let u: f64 = guac.gen();
        let uz: f64 = u * self.zetan;
        if uz < 1.0 {
            return 1;
        }
        if uz < 1.0 + 0.5_f64.powf(self.theta) {
            return 2;
        }
        let scale: f64 = (self.eta * u - self.eta + 1.0).powf(self.alpha);
        1 + (self.n as f64 * scale) as u64
    }

    fn zeta(n: u64, theta: f64) -> f64 {
        let mut sum: f64 = 0.0;

        for i in 0..n {
            let x: f64 = i as f64 + 1.0;
            sum += 1.0 / x.powf(theta);
        }

        sum
    }

    fn init(&mut self) {
        self.zetan = Self::zeta(self.n, self.theta);
        self.zeta2 = Self::zeta(self.theta as u64, 2.0);
        self.eta = (1.0 - 2.0_f64.powf(1.0 - self.theta))
                 / (1.0 - self.zeta2 / self.zetan);
    }
}
