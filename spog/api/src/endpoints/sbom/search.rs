use crate::app_state::AppState;
use crate::search;
use actix_web::{web, HttpResponse};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use spog_model::search::SbomSummary;
use tracing::instrument;
use spog_model::package_info::PackageInfo;
use spog_model::prelude::SummaryEntry;
use spog_model::vuln::SbomReport;
use trustification_api::search::{SearchOptions, SearchResult};
use trustification_auth::client::TokenProvider;
use v11y_model::search::Cves::{Low, Severity};

#[utoipa::path(
    get,
    path = "/api/v1/sbom/search",
    responses(
        (status = OK, description = "Search was performed successfully", body = SearchResultSbom),
    ),
    params(
        search::QueryParams,
        SearchOptions,
    )
)]
#[instrument(skip(state, access_token), err)]
pub async fn search(
    state: web::Data<AppState>,
    params: web::Query<search::QueryParams>,
    options: web::Query<SearchOptions>,
    access_token: Option<BearerAuth>,
) -> actix_web::Result<HttpResponse> {
    let params = params.into_inner();
    log::trace!("Querying SBOM using {}", params.q);
    let data = state
        .search_sbom(
            &params.q,
            params.offset,
            params.limit,
            options.into_inner(),
            &access_token,
        )
        .await?;
    let mut m: Vec<SbomSummary> = Vec::with_capacity(data.result.len());
    for item in data.result {
        let metadata = item.metadata.unwrap_or_default();
        let item = item.document;
        m.push(SbomSummary {
            id: item.id.clone(),
            purl: item.purl,
            name: item.name,
            cpe: item.cpe,
            version: item.version,
            sha256: item.sha256,
            license: item.license,
            snippet: item.snippet,
            classifier: item.classifier,
            supplier: item.supplier.trim_start_matches("Organization: ").to_string(),
            href: format!("/api/v1/sbom?id={}", item.id),
            description: item.description,
            dependencies: item.dependencies,
            vulnerabilities: vec![],
            advisories: None,
            created: item.created,
            metadata,
        });
    }

    let mut result = SearchResult {
        total: Some(data.total),
        result: m,
    };

    // TODO: Use guac to lookup advisories for each sbom!
    search_advisories(state, &mut result.result, &access_token).await;
    Ok(HttpResponse::Ok().json(result))
}

#[instrument(skip_all)]
async fn search_advisories(state: web::Data<AppState>, sboms: &mut Vec<SbomSummary>, provider: &dyn TokenProvider) {
    for sbom in sboms {
        if let Some(q) = sbom.advisories_query() {
            if let Ok(result) = state
                .search_vex(
                    &q,
                    0,
                    100000,
                    SearchOptions {
                        explain: false,
                        metadata: false,
                        summaries: false,
                    },
                    provider,
                )
                .await
            {
                sbom.advisories = Some(result.total as u64);
            }
        }
    }
}

pub async fn sboms_with_vulnerability_summary() -> actix_web::Result<HttpResponse> {

    let mut summary: Vec<(String, &Vec<SummaryEntry>)> = vec![];

    let summaryEntryNone: SummaryEntry = SummaryEntry{
        severity: Severity::None,
        count: 3,
    };
    let summaryEntryLow: SummaryEntry = SummaryEntry{
        severity: Severity::Low,
        count: 5,
    };
    let summaryEntryMedium: SummaryEntry = SummaryEntry{
        severity: Severity::Medium,
        count: 10,
    };
    let summaryEntryHigh: SummaryEntry = SummaryEntry{
        severity: Severity::High,
        count: 4,
    };
    let summaryEntryCritical: SummaryEntry = SummaryEntry{
        severity: Severity::Critical,
        count: 2,
    };
    let entries: Vec<SummaryEntry> = vec![summaryEntryNone, summaryEntryLow,summaryEntryMedium,summaryEntryHigh,summaryEntryCritical];
    summary.push(("sbom1".into(),&entries));
    summary.push(("sbom2".into(),&entries));
    summary.push(("sbom3".into(),&entries));

    Ok(HttpResponse::Ok().json(summary))
}
