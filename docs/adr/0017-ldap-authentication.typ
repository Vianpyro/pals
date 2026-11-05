#import "../template.typ": adr, arch, ext, mermaid, validation

== ADR-0017 — Authentication against the ÉTS directory over LDAP <adr-0017>

*Status:* proposed; to validate with the ÉTS IT service. \
*See also:* #arch("purpose-of-the-document")[architecture], sections #arch("cache")[Cache] and #arch("safe-exam-browser")[Safe Exam Browser]; #adr("0009"), #adr("0012").

=== Context

The architecture assumed #ext("oidc")[OIDC] sign-in through #ext("entra")[Microsoft Entra ID]. The ÉTS IT service suggests #ext("ldap")[LDAP] instead: it connects directly to the school's Active Directory and needs no application registration. Under #ext("seb")[Safe Exam Browser], Entra also required allowing `login.microsoftonline.com` and ran into phone-based multi-factor authentication, which conflicts with the ban on phones during exams.

=== Options considered

#table(
  columns: (3cm, 1fr, 1fr),
  stroke: 0.5pt,
  [*Option*], [*Pros*], [*Cons*],
  [Entra ID (OIDC)],
  [The password never reaches the platform; MFA.],
  [Application registration to obtain; external domain to allow under SEB; phone MFA during exams.],

  [LDAP bind against Active Directory],
  [Suggested and supported by the IT service; the sign-in page stays on the platform's domain, which simplifies SEB.],
  [The password transits through the API; no MFA; sessions are the platform's responsibility.],
)

=== Decision

The API authenticates a user by an LDAP bind against the ÉTS Active Directory, over LDAPS (or StartTLS) only, with the directory's CA certificate pinned in the Ansible configuration. The directory provides identity (identifier, name, e-mail) and nothing else: roles stay per offering (#adr("0012")).

The session is a random identifier in an `HttpOnly`, `Secure`, `SameSite=Lax` cookie, stored in #ext("postgresql")[PostgreSQL], which every API instance already shares (#adr("0010")). A session can therefore be revoked, and no dedicated session store is added.

#mermaid(
  "
  sequenceDiagram
    participant B as Browser
    participant N as nginx
    participant A as API
    participant L as ÉTS Active Directory
    participant D as PostgreSQL
    B->>N: sign-in form (identifier, password)
    N->>A: forwarded (limit_req)
    A->>L: LDAP bind over LDAPS, CA pinned
    alt bind succeeds
      L-->>A: identity (identifier, name, e-mail)
      A->>D: session stored (random identifier)
      A-->>B: cookie HttpOnly, Secure, SameSite=Lax
    else wrong password, expired account or unreachable directory
      A-->>B: distinct error code
    end
    Note over A: the password is never stored nor logged
  ",
  document-context: true,
  width: 100%,
)

There is a single identity implementation, so no identity-provider extension point (#adr("0014")): all LDAP code lives in one module.

=== Consequences

- The password is never stored nor logged, including in error traces; the sign-in route is rate-limited by #ext("nginx-limit-req")[`limit_req`].
- The sign-in route is the only API route reachable without a session (#arch("entry-layer")[Entry layer]).
- The threat model must cover the password transiting through the API.
- Under SEB, only the platform's domain has to be allowed.
- A directory outage prevents new sign-ins, but not sessions already open: students sign in before the exam starts.

#validation(id: "V-0017")[
  Confirm with the IT service the LDAPS URL and CA, the base DN, the identifier attribute, whether a service account is needed for the search, and whether AD groups distinguish students and staff. Check that a wrong password, an expired account and an unreachable directory each produce a distinct error code, and that no password appears in the logs.
]
