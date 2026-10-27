//! 委譲レベル（§15.1.1）。
//!
//! 役割ごとに「AI の自律度」を 4 段階で表す。設定は `project.toml` の `agents.delegation`
//! （全役割の既定）と `agents.roles.<role>`（役割ごとの上書き）で行う（§4.5.1）。
//!
//! §15.1.1 の表をそのまま [`Delegation::can_write`] / [`Delegation::can_chain`] にした。
//! **次タスクの起票は integrator のみ**（付録 A.8）なので、[`Delegation::can_chain`] が
//! `true` でも「誰が」の検査は呼び出し側（M10 の `agents`）が行う。

use serde::{Deserialize, Serialize};

/// 委譲レベル（§15.1.1）。既定は [`Delegation::Propose`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
pub enum Delegation {
    /// 人間がその役割を担う（書き込みしない、連鎖も起こさない）。
    #[serde(rename = "off")]
    Off,
    /// 提案役（**既定**）。`agents/proposals/` への提案のみ。
    #[default]
    #[serde(rename = "propose")]
    Propose,
    /// 自動適用。本文・プロットへ自動適用し、before/after は必ず `agents/proposals/` に残す。
    #[serde(rename = "apply")]
    Apply,
    /// 自律。自動適用に加え、次のタスクを起こせる（integrator のみ、A8）。
    #[serde(rename = "autonomous")]
    Autonomous,
}

impl Delegation {
    /// §15.1.1 の表の文字列（`project.toml` に書く形）。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Propose => "propose",
            Self::Apply => "apply",
            Self::Autonomous => "autonomous",
        }
    }

    /// 本文・プロットへの**書き込み**をしてよいか（§15.1.1「エージェントの書き込み」列）。
    ///
    /// `propose` は `agents/proposals/` への提案だけなので `false`。
    pub fn can_write(self) -> bool {
        matches!(self, Self::Apply | Self::Autonomous)
    }

    /// 次のタスクを起こせるか（§15.1.1「連鎖・次タスク」列）。
    ///
    /// `true` でも起票できるのは **integrator だけ**（A8）。役割の検査は呼び出し側。
    pub fn can_chain(self) -> bool {
        matches!(self, Self::Autonomous)
    }

    /// 人間が起動・採否に関与するか（§15.1.1「人間の関与」列）。`off` 以外は関わらない。
    pub fn is_automatic(self) -> bool {
        !matches!(self, Self::Off)
    }

    /// 文字列から引く（§15.1.1 の 4 値）。表に無ければ `None`。
    ///
    /// `project.toml` の値が壊れていても**ファイル全体を道連れにしない**ため、
    /// [`crate::core::model::project::Agents`] はこの関数で受けて既定値に落とす（§4.3-4）。
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "off" => Some(Self::Off),
            "propose" => Some(Self::Propose),
            "apply" => Some(Self::Apply),
            "autonomous" => Some(Self::Autonomous),
            _ => None,
        }
    }
}
