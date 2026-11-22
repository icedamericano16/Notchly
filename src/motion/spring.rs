//! Damped springs integrated with semi-implicit Euler at ≤ 1/240 s sub-steps.
//! Retargeting keeps velocity, so interrupted animations stay seamless.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpringCfg {
    pub stiffness: f32,
    pub damping: f32,
}

/// iOS-style: response 0.40 s, damping fraction 0.80 (smooth, ~1.5 % overshoot).
pub const ISLAND: SpringCfg = SpringCfg {
    stiffness: 246.7,
    damping: 25.13,
};
/// Response 0.44 s, damping 0.82: height trails the width slightly.
pub const ISLAND_H: SpringCfg = SpringCfg {
    stiffness: 203.9,
    damping: 23.42,
};
/// Response 0.24 s, damping 0.92: buttons, toggles, hover.
pub const SNAPPY: SpringCfg = SpringCfg {
    stiffness: 685.4,
    damping: 48.17,
};
/// Response 0.38 s, damping 0.62: pop-up icons, arrivals, success ticks.
pub const BOUNCY: SpringCfg = SpringCfg {
    stiffness: 273.3,
    damping: 20.5,
};
/// Response 0.50 s, critically damped: lyrics, page slides, colour.
pub const GENTLE: SpringCfg = SpringCfg {
    stiffness: 157.9,
    damping: 25.13,
};

/// Global motion environment derived from settings + Windows animation setting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionEnv {
    pub stiffness_mul: f32,
    pub reduced: bool,
}

impl Default for MotionEnv {
    fn default() -> Self {
        Self {
            stiffness_mul: 1.0,
            reduced: false,
        }
    }
}

impl MotionEnv {
    /// Effective config: speed multiplier (damping × √mul keeps the damping ratio)
    /// and critical damping when motion is reduced.
    pub fn apply(&self, cfg: SpringCfg) -> SpringCfg {
        let k = cfg.stiffness * self.stiffness_mul;
        let mut c = cfg.damping * self.stiffness_mul.sqrt();
        if self.reduced {
            c = c.max(2.0 * k.sqrt());
        }
        SpringCfg {
            stiffness: k,
            damping: c,
        }
    }
}

const SETTLE_X: f32 = 0.05;
const SETTLE_V: f32 = 0.5;
const MAX_SUBSTEP: f32 = 1.0 / 240.0;

#[derive(Clone, Copy, Debug)]
pub struct Spring {
    pub value: f32,
    pub velocity: f32,
    pub target: f32,
    pub cfg: SpringCfg,
    /// Scale of the value used for settle thresholds (e.g. 0.01 for opacity 0..1).
    pub precision: f32,
}

impl Default for Spring {
    fn default() -> Self {
        Self::new(0.0, SNAPPY)
    }
}

impl Spring {
    pub fn new(value: f32, cfg: SpringCfg) -> Self {
        Self {
            value,
            velocity: 0.0,
            target: value,
            cfg,
            precision: 1.0,
        }
    }

    /// A spring for unit-range values (opacity, progress) with tighter settle thresholds.
    pub fn unit(value: f32, cfg: SpringCfg) -> Self {
        Self {
            precision: 0.01,
            ..Self::new(value, cfg)
        }
    }

    pub fn set(&mut self, target: f32) {
        self.target = target;
    }

    pub fn set_cfg(&mut self, target: f32, cfg: SpringCfg) {
        self.target = target;
        self.cfg = cfg;
    }

    pub fn snap(&mut self, value: f32) {
        self.value = value;
        self.target = value;
        self.velocity = 0.0;
    }

    /// Adds an impulse (e.g. release momentum from a drag).
    pub fn kick(&mut self, velocity: f32) {
        self.velocity += velocity;
    }

    pub fn is_settled(&self) -> bool {
        (self.value - self.target).abs() < SETTLE_X * self.precision
            && self.velocity.abs() < SETTLE_V * self.precision
    }

    /// Advances by `dt` seconds. Returns true while still moving.
    pub fn step(&mut self, dt: f32, env: &MotionEnv) -> bool {
        if self.is_settled() {
            self.value = self.target;
            self.velocity = 0.0;
            return false;
        }
        let cfg = env.apply(self.cfg);
        let dt = dt.clamp(0.0, 0.05);
        let steps = (dt / MAX_SUBSTEP).ceil().max(1.0) as usize;
        let h = dt / steps as f32;
        for _ in 0..steps {
            let force = -cfg.stiffness * (self.value - self.target) - cfg.damping * self.velocity;
            self.velocity += force * h;
            self.value += self.velocity * h;
        }
        if self.is_settled() {
            self.value = self.target;
            self.velocity = 0.0;
            return false;
        }
        true
    }
}

/// Closed-form unit step response of a damped spring at time `t` (seconds).
/// Used for staggered reveals so each row is still a spring, not a tween.
pub fn step_response(t: f32, cfg: SpringCfg, env: &MotionEnv) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    let cfg = env.apply(cfg);
    let w0 = cfg.stiffness.sqrt();
    let zeta = cfg.damping / (2.0 * w0);
    if zeta < 1.0 {
        let wd = w0 * (1.0 - zeta * zeta).sqrt();
        let e = (-zeta * w0 * t).exp();
        1.0 - e * ((wd * t).cos() + zeta / (1.0 - zeta * zeta).sqrt() * (wd * t).sin())
    } else {
        let e = (-w0 * t).exp();
        1.0 - e * (1.0 + w0 * t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settle_time(cfg: SpringCfg) -> (f32, f32) {
        let env = MotionEnv::default();
        let mut s = Spring::new(0.0, cfg);
        s.set(100.0);
        let mut t = 0.0;
        let mut peak: f32 = 0.0;
        while s.step(1.0 / 144.0, &env) {
            t += 1.0 / 144.0;
            peak = peak.max(s.value);
            assert!(t < 5.0, "spring never settled");
        }
        (t, peak)
    }

    #[test]
    fn island_overshoot_and_settle() {
        let (t, peak) = settle_time(ISLAND);
        assert!(peak > 100.5 && peak < 106.0, "peak {peak}");
        assert!(t > 0.25 && t < 1.2, "settle {t}");
    }

    #[test]
    fn snappy_barely_overshoots() {
        let (_, peak) = settle_time(SNAPPY);
        assert!(peak < 104.0, "peak {peak}");
    }

    #[test]
    fn bouncy_overshoots_more() {
        let (_, peak) = settle_time(BOUNCY);
        assert!(peak > 105.0, "peak {peak}");
    }

    #[test]
    fn reduced_motion_never_overshoots() {
        let env = MotionEnv {
            stiffness_mul: 1.0,
            reduced: true,
        };
        let mut s = Spring::new(0.0, BOUNCY);
        s.set(100.0);
        for _ in 0..1000 {
            s.step(1.0 / 60.0, &env);
            assert!(s.value <= 100.0001);
        }
    }

    #[test]
    fn retarget_keeps_velocity() {
        let env = MotionEnv::default();
        let mut s = Spring::new(0.0, ISLAND);
        s.set(100.0);
        for _ in 0..10 {
            s.step(1.0 / 120.0, &env);
        }
        let v = s.velocity;
        s.set(0.0);
        assert_eq!(s.velocity, v);
    }

    #[test]
    fn large_dt_is_stable() {
        let env = MotionEnv::default();
        let mut s = Spring::new(0.0, SNAPPY);
        s.set(1.0);
        s.step(10.0, &env); // clamped to 50 ms
        assert!(s.value.is_finite() && s.value < 2.0);
    }

    #[test]
    fn step_response_converges() {
        let env = MotionEnv::default();
        assert!((step_response(3.0, GENTLE, &env) - 1.0).abs() < 0.01);
        assert_eq!(step_response(0.0, GENTLE, &env), 0.0);
    }
}
