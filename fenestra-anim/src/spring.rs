//! A closed-form damped spring: no numeric integration, no state, so any
//! instant is sampled directly (random access).

/// Spring parameters for physical motion.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SpringSpec {
    /// Stiffness (ω² scale): higher = snappier. 170 is gentle, 380 brisk.
    pub stiffness: f32,
    /// Damping: lower overshoots more. Critical damping ≈ 2·√stiffness.
    pub damping: f32,
}

impl SpringSpec {
    /// The closed-form unit step response at `t_secs` seconds with initial
    /// velocity `v0` (in progress units per second): the analytic solution of
    /// a damped spring released at 0 toward 1 — no numeric integration, no
    /// state, so any instant is sampled directly (random access). Returns the
    /// progress (which may overshoot 1.0 when underdamped) and whether the
    /// motion has settled (decay envelope below 0.1%, at which point the
    /// progress is pinned to exactly 1.0).
    ///
    /// `damping` at or above critical is evaluated with the critically-damped
    /// solution.
    pub fn step(self, v0: f32, t_secs: f32) -> (f32, bool) {
        let t = t_secs.max(0.0);
        let stiffness = self.stiffness.max(1.0);
        let damping = self.damping.max(0.1);
        let omega = stiffness.sqrt();
        let zeta = damping / (2.0 * omega);
        let x = if zeta < 1.0 {
            // Underdamped: decaying oscillation (the overshoot case).
            // x(t) = 1 − e^{−ζωt}·(cos(ω_d t) + ((ζω − v0)/ω_d)·sin(ω_d t)),
            // which satisfies x(0) = 0 and x'(0) = v0.
            let wd = omega * (1.0 - zeta * zeta).sqrt();
            let envelope = (-zeta * omega * t).exp();
            1.0 - envelope * ((wd * t).cos() + ((zeta * omega - v0) / wd) * (wd * t).sin())
        } else {
            // Critically damped (and the approximation for overdamped):
            // x(t) = 1 − e^{−ωt}·(1 + (ω − v0)·t), with x'(0) = v0.
            let envelope = (-omega * t).exp();
            1.0 - envelope * (1.0 + (omega - v0) * t)
        };
        let settled = (-zeta.min(1.0) * omega * t).exp() < 0.001;
        if settled { (1.0, true) } else { (x, false) }
    }
}
