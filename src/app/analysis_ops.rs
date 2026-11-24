//! Analysis operations of the application: extracting chromatograms, integrating, calibrating, aligning and scan selection.

use crate::analysis::alignment;
use crate::analysis::calibration;
use crate::analysis::integration;
use crate::analysis::peaks;
use crate::analysis::xic;
use crate::app::state::{App, Cell, WEIGHTS};
use crate::domain::types::{Basis, Kind, Mode};
use crate::domain::{Analyte, MzData, Spectrum};
use crate::support::fmt;

/// Recomputes the areas of all traces of `c` from its integration boundaries.
pub(crate) fn calc_areas(xics: &[Vec<[f64; 2]>], c: &mut Cell, baseline: bool) {
    c.areas = xics
        .iter()
        .map(|x| {
            if c.found {
                integration::integrate(x, c.start, c.end, baseline)
            } else {
                0.0
            }
        })
        .collect();
}

/// Which summary chromatogram of a file to show.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SummaryTrace {
    TotalIon,
    BasePeak,
}

/// Total-ion or base-peak chromatogram of the MS1 (`level` 1) or MS2 scans of a file.
pub(crate) fn chrom(d: &MzData, level: u8, kind: SummaryTrace) -> &[[f64; 2]] {
    match (level, kind) {
        (1, SummaryTrace::TotalIon) => &d.tic1,
        (1, SummaryTrace::BasePeak) => &d.bpc1,
        (_, SummaryTrace::TotalIon) => &d.tic2,
        (_, SummaryTrace::BasePeak) => &d.bpc2,
    }
}

/// Trace used to align retention times: the MS1 total-ion chromatogram if there is one.
pub(crate) fn align_trace(d: &MzData) -> &[[f64; 2]] {
    if d.tic1.len() >= 5 { &d.tic1 } else { &d.tic2 }
}

impl App {
    // ---------------- computation ----------------
    /// Extracts the chromatograms of all targets from file `s`, detects the peaks (except where
    /// boundaries were set by hand) and integrates them.
    pub(crate) fn recompute_sample(&mut self, s: usize) {
        let (na, rt_win) = (self.analytes.len(), self.settings.rt_win);
        let finder = peaks::finder(self.settings.peak_method);
        let baseline = self.effective_baseline();
        let basis = self.settings.basis;
        let traces: Vec<Vec<f64>> = (0..na).map(|a| self.trace_mz(a)).collect();
        let tols: Vec<Vec<f64>> = traces
            .iter()
            .map(|v| v.iter().map(|&m| self.tol_da(m)).collect())
            .collect();
        let sm = &mut self.samples[s];
        let shift = sm.rt_shift;
        sm.xics = traces
            .iter()
            .enumerate()
            .map(|(ai, mzs)| {
                mzs.iter()
                    .enumerate()
                    .map(|(ii, &mz)| {
                        let sp: &[Spectrum] = if basis == Basis::Ms1 {
                            &sm.data.ms1
                        } else {
                            &sm.data.spectra
                        };
                        let mut x = xic::extract(sp, mz, tols[ai][ii]);
                        if shift != 0.0 {
                            for p in x.iter_mut() {
                                p[0] += shift;
                            }
                        }
                        x
                    })
                    .collect()
            })
            .collect();
        if sm.cells.len() != na {
            sm.cells = (0..na).map(|_| Cell::new()).collect();
        }
        for a in 0..na {
            let c = &mut sm.cells[a];
            if !c.manual {
                match finder.find(&sm.xics[a][0], self.analytes[a].rt, rt_win) {
                    Some(p) => {
                        c.start = p.start;
                        c.end = p.end;
                        c.found = true;
                        c.logged = (p.start, p.end);
                    }
                    None => c.found = false,
                }
            }
            calc_areas(&sm.xics[a], c, baseline);
        }
    }

    /// Whether areas are integrated above a baseline: the setting, or required by the peak finder.
    pub fn effective_baseline(&self) -> bool {
        self.settings.baseline || peaks::finder(self.settings.peak_method).subtracts_baseline()
    }

    /// Recompute a single (new or edited) target in every file; its integrations are reset.
    pub(crate) fn recompute_analyte(&mut self, a: usize) {
        let na = self.analytes.len();
        if a >= na {
            return;
        }
        let rt_win = self.settings.rt_win;
        let finder = peaks::finder(self.settings.peak_method);
        let baseline = self.effective_baseline();
        let basis = self.settings.basis;
        let mzs = self.trace_mz(a);
        let tols: Vec<f64> = mzs.iter().map(|&m| self.tol_da(m)).collect();
        for s in 0..self.samples.len() {
            let sm = &mut self.samples[s];
            if sm.xics.len() < na {
                sm.xics.resize(na, vec![]);
            }
            if sm.cells.len() < na {
                sm.cells.resize_with(na, Cell::new);
            }
            let shift = sm.rt_shift;
            sm.xics[a] = mzs
                .iter()
                .enumerate()
                .map(|(ii, &mz)| {
                    let sp: &[Spectrum] = if basis == Basis::Ms1 {
                        &sm.data.ms1
                    } else {
                        &sm.data.spectra
                    };
                    let mut x = xic::extract(sp, mz, tols[ii]);
                    if shift != 0.0 {
                        for p in x.iter_mut() {
                            p[0] += shift;
                        }
                    }
                    x
                })
                .collect();
            let mut c = Cell::new();
            if let Some(p) = finder.find(&sm.xics[a][0], self.analytes[a].rt, rt_win) {
                c.start = p.start;
                c.end = p.end;
                c.found = true;
                c.logged = (p.start, p.end);
            }
            calc_areas(&sm.xics[a], &mut c, baseline);
            sm.cells[a] = c;
        }
        if self.fits.len() < na {
            self.fits.resize(na, None);
        }
        self.refit(a);
    }

    /// Recomputes every file and refits all calibrations.
    pub fn recompute_all(&mut self) {
        self.fits = vec![None; self.analytes.len()];
        for s in 0..self.samples.len() {
            self.recompute_sample(s);
        }
        for a in 0..self.analytes.len() {
            self.refit(a);
        }
    }

    /// Refits the calibration of target `a` from its included standards.
    pub fn refit(&mut self, a: usize) {
        if a >= self.analytes.len() {
            return;
        }
        if self.fits.len() != self.analytes.len() {
            self.fits = vec![None; self.analytes.len()];
        }
        let pts: Vec<(f64, f64)> = self
            .samples
            .iter()
            .filter(|s| {
                s.kind == Kind::Standard
                    && s.cells.len() > a
                    && s.cells[a].conc > 0.0
                    && s.cells[a].found
                    && s.cells[a].include
            })
            .map(|s| (s.cells[a].conc, s.cells[a].areas[0]))
            .collect();
        self.fits[a] = calibration::fit(&pts, self.settings.weighting);
    }

    /// Applies hand-set boundaries to a cell; one editing gesture is one undo step.
    pub fn apply_edit(&mut self, s: usize, a: usize, st: f64, en: f64) {
        if !self.editing {
            self.push_undo("manual integration");
            self.editing = true;
        }
        let baseline = self.effective_baseline();
        let sm = &mut self.samples[s];
        let c = &mut sm.cells[a];
        c.start = st;
        c.end = en;
        c.found = true;
        c.manual = true;
        calc_areas(&sm.xics[a], c, baseline);
        self.refit(a);
    }

    /// Ends a boundary edit and writes the change, with old and new values, to the audit trail.
    pub fn commit(&mut self, s: usize, a: usize) {
        self.editing = false;
        let (st, en, area, logged) = {
            let c = &self.samples[s].cells[a];
            (
                c.start,
                c.end,
                c.areas.first().copied().unwrap_or(0.0),
                c.logged,
            )
        };
        if logged != (st, en) {
            let msg = format!(
                "file={} sha256={} analyte={} start {:.4}->{:.4} end {:.4}->{:.4} area={:.1} reason={}",
                self.samples[s].data.name,
                self.samples[s].data.sha256,
                self.analytes[a].name,
                logged.0,
                st,
                logged.1,
                en,
                area,
                if self.reason.trim().is_empty() {
                    "NONE GIVEN"
                } else {
                    self.reason.trim()
                }
            );
            self.samples[s].cells[a].logged = (st, en);
            self.audit.log("MANUAL_INTEGRATION", &msg);
        }
    }

    /// Applies changed extraction settings (tolerance, RT window, weighting, baseline) to all files.
    pub fn apply_settings(&mut self) {
        self.audit.log(
            "PARAMETERS",
            &format!(
                "peak_method={} tol={}{} rt_window={} weighting={} baseline={} ion_ratio_tol={}%",
                self.settings.peak_method.key(),
                self.settings.tol,
                if self.settings.tol_ppm { "ppm" } else { "Da" },
                self.settings.rt_win,
                WEIGHTS[self.settings.weighting as usize],
                self.settings.baseline,
                self.settings.ratio_tol
            ),
        );
        self.recompute_all();
        self.settings_dirty = false;
    }

    // ---------------- retention-time alignment ----------------
    /// Retention-time shift that aligns file `s` to the reference file; `None` if one of them has too few scans.
    pub(crate) fn compute_shift(&self, s: usize) -> Option<f64> {
        let r = self.samples.get(self.align_ref)?;
        let x = &self.samples[s];
        let (tr, tx) = (align_trace(&r.data), align_trace(&x.data));
        if tr.len() < 5 || tx.len() < 5 {
            return None;
        }
        Some(alignment::estimate_shift(tr, tx, self.settings.align_max) + r.rt_shift)
    }

    /// Sets the retention-time shift of file `s`.
    ///
    /// Its manual integrations are reset because they refer to the shifted times.
    pub(crate) fn set_shift(&mut self, s: usize, shift: f64, why: &str) {
        let old = self.samples[s].rt_shift;
        {
            let sm = &mut self.samples[s];
            sm.rt_shift = shift;
            sm.shift_ui = shift;
            for c in sm.cells.iter_mut() {
                c.manual = false;
            }
        }
        self.recompute_sample(s);
        for a in 0..self.analytes.len() {
            self.refit(a);
        }
        let msg = format!(
            "file={} sha256={} shift {:.4}->{:.4} min reason={} (manual integrations of this file reset)",
            self.samples[s].data.name, self.samples[s].data.sha256, old, shift, why
        );
        self.audit.log("RT_SHIFT", &msg);
    }

    /// Aligns every file to the reference file.
    pub(crate) fn align_all(&mut self) {
        let n = self.samples.len();
        if n < 2 {
            self.status = "Need at least two files to align.".into();
            return;
        }
        if self.align_ref >= n {
            self.align_ref = 0;
        }
        self.push_undo("align all");
        for s in 0..n {
            if s == self.align_ref {
                continue;
            }
            if let Some(sh) = self.compute_shift(s) {
                self.set_shift(s, sh, "align all (TIC cross-correlation)");
            }
        }
        self.status = format!(
            "Aligned {} files to {}",
            n - 1,
            self.samples[self.align_ref].data.name
        );
    }

    /// Removes all retention-time shifts.
    pub(crate) fn clear_alignment(&mut self) {
        self.push_undo("clear alignment");
        for s in 0..self.samples.len() {
            if self.samples[s].rt_shift != 0.0 {
                self.set_shift(s, 0.0, "clear alignment");
            }
        }
        self.status = "Alignment cleared".into();
    }

    // ---------------- results ----------------
    /// Concentration of target `a` in file `s` according to the calibration.
    ///
    /// Returns `None` in areas-only mode, without calibration and without a peak.
    pub fn conc_of(&self, s: usize, a: usize) -> Option<f64> {
        if self.settings.mode == Mode::Areas {
            return None;
        }
        let f = self.fits.get(a).copied().flatten()?;
        let c = self.samples.get(s)?.cells.get(a)?;
        if !c.found || f.m == 0.0 {
            return None;
        }
        Some((c.areas[0] - f.b) / f.m)
    }

    /// Mean ratio of qualifier ion `ion` to the quantifier ion over the included standards of target `a`.
    pub(crate) fn ref_ratio(&self, a: usize, ion: usize) -> Option<f64> {
        let v: Vec<f64> = self
            .samples
            .iter()
            .filter(|s| {
                s.kind == Kind::Standard
                    && s.cells.len() > a
                    && s.cells[a].conc > 0.0
                    && s.cells[a].found
                    && s.cells[a].include
                    && s.cells[a].areas[0] > 0.0
            })
            .map(|s| s.cells[a].areas[ion] / s.cells[a].areas[0])
            .collect();
        if v.is_empty() {
            None
        } else {
            Some(v.iter().sum::<f64>() / v.len() as f64)
        }
    }

    /// Quality flags of one result (not detected, manual, ion-ratio failures, calibration range, accuracy) as text.
    pub(crate) fn flags(&self, s: usize, a: usize) -> String {
        let sm = &self.samples[s];
        let c = match sm.cells.get(a) {
            Some(c) => c,
            None => return String::new(),
        };
        let mut f: Vec<String> = vec![];
        if !c.found || c.areas[0] <= 0.0 {
            return "ND".into();
        }
        if c.manual {
            f.push("manual".into());
        }
        if sm.kind == Kind::Blank {
            f.push("PEAK IN BLANK".into());
        }
        if let (Some(fit), Some(cc)) = (self.fits.get(a).copied().flatten(), self.conc_of(s, a)) {
            if sm.kind != Kind::Standard && (cc < fit.lo || cc > fit.hi) {
                f.push("outside cal range".into());
            }
            if sm.kind == Kind::Standard && c.conc > 0.0 && ((cc - c.conc) / c.conc).abs() > 0.15 {
                f.push("accuracy >15%".into());
            }
        }
        for i in 1..c.areas.len() {
            if let Some(r) = self.ref_ratio(a, i) {
                let obs = c.areas[i] / c.areas[0];
                if r > 0.0 && ((obs - r) / r).abs() * 100.0 > self.settings.ratio_tol {
                    f.push(format!("ion{} ratio fail", i + 1));
                }
            }
        }
        f.join("; ")
    }

    /// m/z chromatogram of the "custom m/z" view for file `f` (aligned retention times).
    pub fn quick_trace(&self, f: usize) -> Vec<[f64; 2]> {
        let sm = &self.samples[f];
        let sp: &[Spectrum] = if self.quick_level == 1 {
            &sm.data.ms1
        } else {
            &sm.data.spectra
        };
        let mut x = xic::extract(sp, self.quick_mz, self.tol_da(self.quick_mz));
        if sm.rt_shift != 0.0 {
            for p in x.iter_mut() {
                p[0] += sm.rt_shift;
            }
        }
        x
    }

    /// m/z values traced for target `a`: the product ions (MS2 basis) or the precursor (MS1 basis).
    pub(crate) fn trace_mz(&self, a: usize) -> Vec<f64> {
        match self.settings.basis {
            Basis::Ms2 => self.analytes[a].ions.clone(),
            Basis::Ms1 => vec![self.analytes[a].precursor.unwrap_or(0.0)],
        }
    }

    /// Switches the quantified signal (undoable) and recomputes everything.
    pub fn set_basis(&mut self, b: Basis) {
        if self.settings.basis == b {
            return;
        }
        self.push_undo("quantification signal");
        self.settings.basis = b;
        self.audit.log(
            "QUANT_BASIS",
            if b == Basis::Ms1 {
                "MS1 precursor area"
            } else {
                "MS2 product-ion area"
            },
        );
        self.recompute_all();
        if b == Basis::Ms1 && self.analytes.iter().any(|a| a.precursor.is_none()) {
            self.status = "Some targets have no precursor m/z (add p:<m/z> in the table); they cannot be quantified on the MS1 signal.".into();
        }
    }

    /// MS2 scan (index into `spectra`) closest in time to `rt` (aligned minutes) in file `f`.
    pub fn nearest_ms2(&self, f: usize, rt: f64) -> Option<usize> {
        let sm = self.samples.get(f)?;
        let sp = &sm.data.spectra;
        if sp.is_empty() {
            return None;
        }
        let t = rt - sm.rt_shift;
        let j = sp.partition_point(|x| x.rt < t);
        let (c0, c1) = (j.saturating_sub(1), j.min(sp.len() - 1));
        Some(if (sp[c1].rt - t).abs() < (sp[c0].rt - t).abs() {
            c1
        } else {
            c0
        })
    }

    /// MS2 scan inside the integration window with the strongest quantifier product ion.
    pub fn apex_scan(&self, s: usize, a: usize) -> Option<usize> {
        let sm = self.samples.get(s)?;
        let c = sm.cells.get(a)?;
        if !c.found {
            return None;
        }
        let ion = *self.analytes.get(a)?.ions.first()?;
        let tol = self.tol_da(ion);
        let (t0, t1) = (c.start - sm.rt_shift, c.end - sm.rt_shift);
        let sp = &sm.data.spectra;
        let (mut best, mut top) = (None, 0.0f64);
        for (j, spec) in sp
            .iter()
            .enumerate()
            .skip(sp.partition_point(|x| x.rt < t0))
        {
            if spec.rt > t1 {
                break;
            }
            let v = xic::intensity_near(&spec.mz, &spec.inten, ion, tol);
            if v > top {
                top = v;
                best = Some(j);
            }
        }
        best
    }

    /// Pins the MS2 scan closest to `rt` in file `f` in the spectrum panel.
    pub fn pin_scan(&mut self, f: usize, rt: f64) {
        if let Some(i) = self.nearest_ms2(f, rt) {
            self.pinned_scan = Some((f, i));
        }
    }

    /// Which MS2 scan the spectrum panel shows: hovered > pinned > apex of the selected target in the current file.
    pub fn spectrum_pick(&self) -> Option<(usize, usize, &'static str)> {
        if self.follow_hover {
            if let Some((f, rt)) = self.hover {
                if let Some(i) = self.nearest_ms2(f, rt) {
                    return Some((f, i, "hovered"));
                }
            }
        }
        if let Some((f, i)) = self.pinned_scan {
            if f < self.samples.len() && i < self.samples[f].data.spectra.len() {
                return Some((f, i, "pinned"));
            }
        }
        let s = self.cur()?;
        if self.sel_a < self.analytes.len() {
            return self
                .apex_scan(s, self.sel_a)
                .map(|i| (s, i, "apex of the selected target"));
        }
        None
    }

    // ---------------- calibration setup (standards per target, replicates) ----------------
    /// Gives every selected file the concentration `conc` for `target` (conc = 0 removes it) and marks them as standards.
    pub fn assign_standards(&mut self, target: usize, conc: f64) {
        let files = self.shown();
        if files.is_empty() || target >= self.analytes.len() {
            return;
        }
        self.push_undo(if conc > 0.0 {
            "assign standards"
        } else {
            "remove concentrations"
        });
        for &f in &files {
            let sm = &mut self.samples[f];
            if let Some(c) = sm.cells.get_mut(target) {
                c.conc = conc;
            }
            if conc > 0.0 {
                sm.kind = Kind::Standard;
            }
        }
        self.refit(target);
        let names: Vec<String> = files
            .iter()
            .map(|&f| self.samples[f].data.name.clone())
            .collect();
        self.audit.log(
            "STANDARDS_ASSIGNED",
            &format!(
                "target={} conc={} files={}",
                self.analytes[target].name,
                conc,
                names.join(",")
            ),
        );
        self.status = format!(
            "{} file(s): {} = {} {}",
            files.len(),
            self.analytes[target].name,
            fmt::sci(conc),
            self.settings.units
        );
    }

    /// Turns the m/z of the "custom m/z" view into a permanent target (MS2 level: product ion, MS1 level: precursor).
    pub fn add_quick_target(&mut self) {
        let mz = self.quick_mz;
        if mz <= 0.0 {
            return;
        }
        self.push_undo("add target");
        let n = self.analytes.len();
        let prec = if self.quick_level == 1 {
            Some(mz)
        } else {
            None
        };
        self.analytes.push(Analyte {
            name: format!("m/z {}", fmt::mz4(mz)),
            rt: None,
            ions: vec![mz],
            precursor: prec,
        });
        self.fits.push(None);
        self.recompute_analyte(n);
        self.rows_dirty = true;
        self.audit.log(
            "TARGET_ADDED",
            &format!(
                "#{} from quick XIC m/z {} MS{}",
                n + 1,
                fmt::mz4(mz),
                self.quick_level
            ),
        );
        self.status = format!(
            "Added target m/z {} - edit its name and RT in Edit → Target table",
            fmt::mz4(mz)
        );
    }
}
