Wealth tax, stamp duty, and CGT now correctly reduce household net income so distributional impacts and winners/losers outputs are non-zero. Previously these were tallied in total_tax but never subtracted from disposable income.

CGT no longer fabricates gains from investment income: a `capital_gains` field has been added to `Person` (defaulting to zero, since no current survey records realised gains). The `realisation_rate` proxy has been removed.

Fixed a `TenureType` CSV round-trip bug where `to_rf_code` and `from_frs_code` used different numbering, which scrambled tenure on load and caused the EFRS property wealth imputation to assign property to renters. After the fix, SDLT comes out at £11.6bn (HMRC actual ~£12bn). Property wealth predictions for renters are now explicitly zeroed post-imputation.

WAS standalone loader now uses `property_wealth` as a proxy for `main_residence_value` so stamp duty reforms are non-zero on WAS data.

WAS and LCFS training data subsampled to ~1500 rows to reduce memory and disk usage during EFRS extraction.
