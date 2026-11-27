# Design

## D1. Только стабильные фичи
Job запускает `phgettags` и `phfixdto` — они проходят 12/12. Полный набор
phtools содержит падения, воспроизводимые с настоящим exiftool 13.55, и в
гейт не входит.

## D2. Pinned checkout
phtools клонируется на commit `8e3f18c…`; обновление — отдельным изменением.

## D3. Ruby 3.4
`ruby/setup-ruby@v1`, в Gemfile phtools добавляются `erb` и `mutex_m`
(удалены из stdlib Ruby 3.4).

## D4. Vendor bundle
`bundle config path vendor/bundle` — зависимости не смешиваются с системными.

## Risks

- phtools репозиторий должен быть публичным; иначе нужен token.
- Онлайн-зависимость CI от GitHub checkout.
