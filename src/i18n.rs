//! Internationalisation of the **OxideForms** UI chrome.
//!
//! Only the application's own strings — headings, buttons, labels, prompts,
//! hints — are translated. The form content that lives in the `.json` definition
//! files (titles, descriptions, field labels, option values, placeholders) is
//! always rendered verbatim, in whatever language its author wrote it.
//!
//! The active language is remembered per visitor in an `oxideforms_lang` cookie
//! and, absent that, inferred from the `Accept-Language` request header. Both
//! are resolved in [`resolve_lang`].

/// The UI languages OxideForms ships translations for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    /// `en-US` — the default language.
    En,
    /// `pt-BR`
    Pt,
    /// `de`
    De,
    /// `fr`
    Fr,
    /// `zh` (Simplified Chinese)
    Zh,
    /// `es`
    Es,
}

impl Lang {
    /// Every supported language, in the order the header switcher lists them.
    pub const ALL: [Lang; 6] = [Lang::En, Lang::Pt, Lang::De, Lang::Fr, Lang::Zh, Lang::Es];

    /// BCP-47-ish code used in `<html lang="…">`, the cookie and the switcher.
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en-US",
            Lang::Pt => "pt-BR",
            Lang::De => "de",
            Lang::Fr => "fr",
            Lang::Zh => "zh",
            Lang::Es => "es",
        }
    }

    /// The language's own name, as shown in the header switcher.
    pub fn endonym(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Pt => "Português (BR)",
            Lang::De => "Deutsch",
            Lang::Fr => "Français",
            Lang::Zh => "中文",
            Lang::Es => "Español",
        }
    }

    /// Best-effort parse of a language code (a cookie value or a switcher value).
    /// Accepts bare tags (`de`), region tags (`pt-BR`) and common aliases.
    pub fn parse(code: &str) -> Option<Lang> {
        let c = code.trim().to_ascii_lowercase().replace('_', "-");
        Some(match c.as_str() {
            "en" | "en-us" | "en-gb" | "en-au" | "en-ca" => Lang::En,
            "pt" | "pt-br" | "pt-pt" => Lang::Pt,
            "de" | "de-de" | "de-at" | "de-ch" => Lang::De,
            "fr" | "fr-fr" | "fr-be" | "fr-ca" | "fr-ch" => Lang::Fr,
            "zh" | "zh-cn" | "zh-sg" | "zh-hans" | "zh-tw" | "zh-hant" => Lang::Zh,
            "es" | "es-es" | "es-mx" | "es-ar" | "es-us" => Lang::Es,
            _ => return None,
        })
    }
}

/// Pick the best supported language from an `Accept-Language` header value,
/// honouring `q` weights. Returns `None` when no supported language is listed.
///
/// A bare `*` (any language) is ignored here; [`resolve_lang`] decides the
/// default. Tokens that don't map to a supported language are skipped.
fn from_accept_language(header: &str) -> Option<Lang> {
    let mut scored: Vec<(f32, Lang)> = Vec::new();
    for part in header.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let (code, q) = match part.split_once(';') {
            Some((code, q)) => {
                let q = q
                    .trim()
                    .strip_prefix("q=")
                    .and_then(|s| s.trim().parse::<f32>().ok())
                    .unwrap_or(1.0);
                (code.trim(), q)
            }
            None => (part, 1.0),
        };
        if code == "*" || q <= 0.0 {
            continue;
        }
        if let Some(lang) = Lang::parse(code) {
            scored.push((q, lang));
        }
    }
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().next().map(|(_, lang)| lang)
}

/// Resolve the language to use for a request:
///
/// 1. the remembered `oxideforms_lang` cookie, if it holds a supported language;
/// 2. otherwise the best match from `Accept-Language`;
/// 3. otherwise English (`en-US`).
pub fn resolve_lang(cookie: Option<&str>, accept_language: Option<&str>) -> Lang {
    if let Some(code) = cookie.and_then(Lang::parse) {
        return code;
    }
    if let Some(lang) = accept_language.and_then(from_accept_language) {
        return lang;
    }
    Lang::En
}

/// All the translatable OxideForms UI strings for one language, plus a few
/// helpers for strings that depend on a runtime value (form title, uuid, count).
///
/// Cheap to copy: it is just a [`Lang`] and a handful of `&'static str` pointers.
#[derive(Clone, Copy)]
pub struct T {
    pub lang: Lang,
    /// Value for the `<html lang="…">` attribute and the language cookie.
    pub code: &'static str,
    /// Accessible label for the header language switcher.
    pub lang_label: &'static str,

    // footer
    pub footer_source: &'static str,
    pub footer_made_by: &'static str,
    pub github_aria: &'static str,

    // form page
    pub fix_below: &'static str,
    pub required_hint: &'static str,
    pub submit: &'static str,
    pub next: &'static str,
    pub back: &'static str,
    /// Shown in a textarea that didn't define its own placeholder.
    pub default_textarea_placeholder: &'static str,
    pub already_submitted: &'static str,

    // success page
    pub received_word: &'static str,
    pub success_heading: &'static str,
    pub submit_another: &'static str,
    pub view_submissions: &'static str,

    // admin login page
    pub admin_word: &'static str,
    pub admin_heading: &'static str,
    pub admin_password_label: &'static str,
    pub unlock: &'static str,
    pub back_to_form: &'static str,
    pub admin_not_configured_view: &'static str,
    pub admin_not_configured: &'static str,
    pub wrong_password: &'static str,

    // submissions (admin) page
    pub submissions_word: &'static str,
    pub refresh: &'static str,
    pub log_out: &'static str,
    pub view_form: &'static str,
    pub no_submissions: &'static str,
    pub no_submissions_hint: &'static str,
    pub submitted_col: &'static str,

    // not-found page
    pub not_found_word: &'static str,
    pub not_found_heading: &'static str,
    pub view_on_github: &'static str,
}

impl T {
    /// Build the table of UI strings for `lang`.
    pub fn for_lang(lang: Lang) -> T {
        match lang {
            Lang::En => T {
                lang,
                code: "en-US",
                lang_label: "Language",
                footer_source: "source code",
                footer_made_by: "made by",
                github_aria: "View the GitHub repository",
                fix_below: "Please fix the following:",
                required_hint: "Fields marked * are required.",
                submit: "Submit response",
                next: "Next",
                back: "Back",
                default_textarea_placeholder: "Type your answer…",
                already_submitted: "You have already submitted this form.",
                received_word: "Received",
                success_heading: "Response received",
                submit_another: "Submit another",
                view_submissions: "View submissions",
                admin_word: "Admin",
                admin_heading: "Administrator access",
                admin_password_label: "Admin password",
                unlock: "Unlock",
                back_to_form: "← Back to form",
                admin_not_configured_view: "Admin is not configured. Set the ADMIN_PASSWORD environment variable to enable this view.",
                admin_not_configured: "Admin is not configured. Set the ADMIN_PASSWORD environment variable.",
                wrong_password: "Incorrect password. Please try again.",
                submissions_word: "Submissions",
                refresh: "↻ Refresh",
                log_out: "Log out",
                view_form: "View form",
                no_submissions: "No submissions yet",
                no_submissions_hint: "Responses will appear here as soon as someone submits the form.",
                submitted_col: "Submitted",
                not_found_word: "Not found",
                not_found_heading: "Form not found",
                view_on_github: "View on GitHub",
            },
            Lang::Pt => T {
                lang,
                code: "pt-BR",
                lang_label: "Idioma",
                footer_source: "código-fonte",
                footer_made_by: "feito por",
                github_aria: "Ver o repositório no GitHub",
                fix_below: "Corrija os itens abaixo:",
                required_hint: "Campos marcados com * são obrigatórios.",
                submit: "Enviar resposta",
                next: "Próximo",
                back: "Voltar",
                default_textarea_placeholder: "Digite sua resposta…",
                already_submitted: "Você já enviou este formulário.",
                received_word: "Recebido",
                success_heading: "Resposta recebida",
                submit_another: "Enviar outra",
                view_submissions: "Ver respostas",
                admin_word: "Admin",
                admin_heading: "Acesso de administrador",
                admin_password_label: "Senha do administrador",
                unlock: "Desbloquear",
                back_to_form: "← Voltar ao formulário",
                admin_not_configured_view: "O admin não está configurado. Defina a variável de ambiente ADMIN_PASSWORD para habilitar esta visão.",
                admin_not_configured: "O admin não está configurado. Defina a variável de ambiente ADMIN_PASSWORD.",
                wrong_password: "Senha incorreta. Tente novamente.",
                submissions_word: "Respostas",
                refresh: "↻ Atualizar",
                log_out: "Sair",
                view_form: "Ver formulário",
                no_submissions: "Nenhuma resposta ainda",
                no_submissions_hint: "As respostas aparecerão aqui assim que alguém enviar o formulário.",
                submitted_col: "Enviada",
                not_found_word: "Não encontrado",
                not_found_heading: "Formulário não encontrado",
                view_on_github: "Ver no GitHub",
            },
            Lang::De => T {
                lang,
                code: "de",
                lang_label: "Sprache",
                footer_source: "Quellcode",
                footer_made_by: "entwickelt von",
                github_aria: "GitHub-Repository ansehen",
                fix_below: "Bitte behebe das Folgende:",
                required_hint: "Felder mit * sind Pflichtfelder.",
                submit: "Antwort absenden",
                next: "Weiter",
                back: "Zurück",
                default_textarea_placeholder: "Gib deine Antwort ein…",
                already_submitted: "Du hast dieses Formular bereits abgeschickt.",
                received_word: "Empfangen",
                success_heading: "Antwort erhalten",
                submit_another: "Weitere Antwort senden",
                view_submissions: "Antworten ansehen",
                admin_word: "Admin",
                admin_heading: "Administrator-Zugriff",
                admin_password_label: "Admin-Passwort",
                unlock: "Entsperren",
                back_to_form: "← Zurück zum Formular",
                admin_not_configured_view: "Admin ist nicht konfiguriert. Setze die Umgebungsvariable ADMIN_PASSWORD, um diese Ansicht zu aktivieren.",
                admin_not_configured: "Admin ist nicht konfiguriert. Setze die Umgebungsvariable ADMIN_PASSWORD.",
                wrong_password: "Falsches Passwort. Bitte versuche es erneut.",
                submissions_word: "Antworten",
                refresh: "↻ Aktualisieren",
                log_out: "Abmelden",
                view_form: "Formular ansehen",
                no_submissions: "Noch keine Antworten",
                no_submissions_hint: "Antworten erscheinen hier, sobald jemand das Formular absendet.",
                submitted_col: "Gesendet",
                not_found_word: "Nicht gefunden",
                not_found_heading: "Formular nicht gefunden",
                view_on_github: "Auf GitHub ansehen",
            },
            Lang::Fr => T {
                lang,
                code: "fr",
                lang_label: "Langue",
                footer_source: "code source",
                footer_made_by: "créé par",
                github_aria: "Voir le dépôt GitHub",
                fix_below: "Veuillez corriger ce qui suit :",
                required_hint: "Les champs marqués d'un * sont obligatoires.",
                submit: "Envoyer la réponse",
                next: "Suivant",
                back: "Retour",
                default_textarea_placeholder: "Saisissez votre réponse…",
                already_submitted: "Vous avez déjà soumis ce formulaire.",
                received_word: "Reçu",
                success_heading: "Réponse reçue",
                submit_another: "Envoyer une autre",
                view_submissions: "Voir les réponses",
                admin_word: "Admin",
                admin_heading: "Accès administrateur",
                admin_password_label: "Mot de passe administrateur",
                unlock: "Déverrouiller",
                back_to_form: "← Retour au formulaire",
                admin_not_configured_view: "L'administration n'est pas configurée. Définissez la variable d'environnement ADMIN_PASSWORD pour activer cette vue.",
                admin_not_configured: "L'administration n'est pas configurée. Définissez la variable d'environnement ADMIN_PASSWORD.",
                wrong_password: "Mot de passe incorrect. Veuillez réessayer.",
                submissions_word: "Réponses",
                refresh: "↻ Actualiser",
                log_out: "Se déconnecter",
                view_form: "Voir le formulaire",
                no_submissions: "Aucune réponse pour le moment",
                no_submissions_hint: "Les réponses apparaîtront ici dès que quelqu'un soumettra le formulaire.",
                submitted_col: "Soumise",
                not_found_word: "Non trouvé",
                not_found_heading: "Formulaire introuvable",
                view_on_github: "Voir sur GitHub",
            },
            Lang::Zh => T {
                lang,
                code: "zh",
                lang_label: "语言",
                footer_source: "源代码",
                footer_made_by: "由",
                github_aria: "查看 GitHub 仓库",
                fix_below: "请修正以下问题：",
                required_hint: "标有 * 的字段为必填项。",
                submit: "提交回答",
                next: "下一页",
                back: "返回",
                default_textarea_placeholder: "请输入您的回答…",
                already_submitted: "您已提交过此表单。",
                received_word: "已接收",
                success_heading: "已收到回答",
                submit_another: "再次提交",
                view_submissions: "查看回答",
                admin_word: "管理员",
                admin_heading: "管理员访问",
                admin_password_label: "管理员密码",
                unlock: "解锁",
                back_to_form: "← 返回表单",
                admin_not_configured_view: "未配置管理员。请设置 ADMIN_PASSWORD 环境变量以启用此视图。",
                admin_not_configured: "未配置管理员。请设置 ADMIN_PASSWORD 环境变量。",
                wrong_password: "密码错误。请重试。",
                submissions_word: "回答",
                refresh: "↻ 刷新",
                log_out: "退出",
                view_form: "查看表单",
                no_submissions: "暂无回答",
                no_submissions_hint: "当有人提交表单后，回答将显示在此处。",
                submitted_col: "提交时间",
                not_found_word: "未找到",
                not_found_heading: "未找到表单",
                view_on_github: "在 GitHub 查看",
            },
            Lang::Es => T {
                lang,
                code: "es",
                lang_label: "Idioma",
                footer_source: "código fuente",
                footer_made_by: "hecho por",
                github_aria: "Ver el repositorio en GitHub",
                fix_below: "Por favor, corrige lo siguiente:",
                required_hint: "Los campos marcados con * son obligatorios.",
                submit: "Enviar respuesta",
                next: "Siguiente",
                back: "Atrás",
                default_textarea_placeholder: "Escribe tu respuesta…",
                already_submitted: "Ya has enviado este formulario.",
                received_word: "Recibido",
                success_heading: "Respuesta recibida",
                submit_another: "Enviar otra",
                view_submissions: "Ver respuestas",
                admin_word: "Admin",
                admin_heading: "Acceso de administrador",
                admin_password_label: "Contraseña de administrador",
                unlock: "Desbloquear",
                back_to_form: "← Volver al formulario",
                admin_not_configured_view: "El admin no está configurado. Define la variable de entorno ADMIN_PASSWORD para habilitar esta vista.",
                admin_not_configured: "El admin no está configurado. Define la variable de entorno ADMIN_PASSWORD.",
                wrong_password: "Contraseña incorrecta. Inténtalo de nuevo.",
                submissions_word: "Respuestas",
                refresh: "↻ Actualizar",
                log_out: "Cerrar sesión",
                view_form: "Ver formulario",
                no_submissions: "Aún no hay respuestas",
                no_submissions_hint: "Las respuestas aparecerán aquí en cuanto alguien envíe el formulario.",
                submitted_col: "Enviada",
                not_found_word: "No encontrado",
                not_found_heading: "Formulario no encontrado",
                view_on_github: "Ver en GitHub",
            },
        }
    }

    /// The list of languages offered by the header switcher.
    pub fn languages(&self) -> &'static [Lang] {
        &Lang::ALL
    }

    /// "Thanks — your response to `{title}` has been recorded."
    pub fn thanks(&self, title: &str) -> String {
        match self.lang {
            Lang::En => format!("Thanks — your response to {title} has been recorded."),
            Lang::Pt => format!("Obrigado — sua resposta para {title} foi registrada."),
            Lang::De => format!("Danke — deine Antwort zu {title} wurde erfasst."),
            Lang::Fr => format!("Merci — votre réponse à {title} a été enregistrée."),
            Lang::Zh => format!("谢谢 — 您对 {title} 的回答已记录。"),
            Lang::Es => format!("Gracias — tu respuesta a {title} ha sido registrada."),
        }
    }

    /// "Enter the admin password to view submissions for “{title}”."
    pub fn admin_prompt(&self, title: &str) -> String {
        match self.lang {
            Lang::En => format!("Enter the admin password to view submissions for “{title}”."),
            Lang::Pt => format!("Digite a senha do administrador para ver as respostas de “{title}”."),
            Lang::De => format!("Gib das Admin-Passwort ein, um die Antworten für “{title}” zu sehen."),
            Lang::Fr => format!("Saisissez le mot de passe administrateur pour voir les réponses de « {title} »."),
            Lang::Zh => format!("请输入管理员密码以查看“{title}”的回答。"),
            Lang::Es => format!("Introduce la contraseña de administrador para ver las respuestas de “{title}”."),
        }
    }

    /// "There is no form at `/{uuid}`."
    pub fn not_found_body(&self, uuid: &str) -> String {
        match self.lang {
            Lang::En => format!("There is no form at /{uuid}."),
            Lang::Pt => format!("Não há nenhum formulário em /{uuid}."),
            Lang::De => format!("Unter /{uuid} existiert kein Formular."),
            Lang::Fr => format!("Aucun formulaire à /{uuid}."),
            Lang::Zh => format!("/{uuid} 不存在表单。"),
            Lang::Es => format!("No hay ningún formulario en /{uuid}."),
        }
    }

    /// "“{label}” is required."
    pub fn required_error(&self, label: &str) -> String {
        match self.lang {
            Lang::En => format!("“{label}” is required."),
            Lang::Pt => format!("“{label}” é obrigatório."),
            Lang::De => format!("„{label}“ ist ein Pflichtfeld."),
            Lang::Fr => format!("« {label} » est obligatoire."),
            Lang::Zh => format!("“{label}” 为必填项。"),
            Lang::Es => format!("“{label}” es obligatorio."),
        }
    }

    /// "{title} · {n} response(s)" — the subtitle line of the submissions page.
    pub fn submissions_subtitle(&self, title: &str, n: usize) -> String {
        match self.lang {
            Lang::En => format!("{title} · {n} response{}", if n == 1 { "" } else { "s" }),
            Lang::Pt => format!("{title} · {n} resposta{}", if n == 1 { "" } else { "s" }),
            Lang::De => format!("{title} · {n} Antwort{}", if n == 1 { "" } else { "en" }),
            Lang::Fr => format!("{title} · {n} réponse{}", if n == 1 { "" } else { "s" }),
            Lang::Zh => format!("{title} · {n} 条回答"),
            Lang::Es => format!("{title} · {n} respuesta{}", if n == 1 { "" } else { "s" }),
        }
    }
}
