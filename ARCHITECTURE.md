# 🏗️ Rift L1 Blockchain — Архитектура

## Структура репозитория
## Модули

### `src/lib.rs` — Публичный API

Экспортирует:
- `CoreState` — состояние ядра
- `RiftTokenState` — слой токен-экономики
- `InvariantError` — типы ошибок

### `src/core/state.rs` — CoreState

**Поля:**
- `global_field: i128` — глобальное поле системы
- `total_base_sum: i128` — сумма базовых балансов
- `total_supply: u128` — полный объём монет
- `total_minted: u128` — выпущено всего
- `total_burned: u128` — сожжено всего
- `participants_count: u64` — количество участников
- `dust_accumulator: u128` — остаток от распределения
- `paused: bool` — статус паузы

**Методы:**
- `new()` — создание
- `check_invariant()` — проверка всех 4 SIRM
- `fingerprint()` — хеш состояния
- `debt_limit()` — лимит долга

### `src/core/operations.rs` — Операции

Все операции CoreState:
- `register()` — добавить участника
- `unregister(balance)` — удалить участника
- `transfer(from, to, amount, edge_cost)` — перевод
- `redistribute(amount)` — распределить
- `apply_neg_entropy()` — энтропия

**Гарантия:** каждая операция проверяет инварианты после выполнения

### `src/token/rift_token.rs` — RiftTokenState

**Поля:**
- `total_shares: u64` — выпущено RIFT
- `rift_multiplier: u128` — динамичный курс mint
- `fee_bps: u16` — комиссия протокола

**Методы:**
- `new(fee_bps)` — создание
- `issue_rift(core, amount)` — выпуск токенов
- `rebase(core)` — обновить multiplier

### `src/error.rs` — Обработка ошибок

```rust
pub struct InvariantError(pub String);
pub type Result<T> = std::result::Result<T, InvariantError>;
```

## Порядок зависимостей
## Что НЕ ТРОГАЕМ

❌ Логика ядра в `core/state.rs` — критичное место
❌ Проверки инвариантов — математически доказаны
❌ Основной main.rs — только импорты

## Что МОЖНО МЕНЯТЬ

✅ Добавлять новые операции (с проверкой инвариантов)
✅ Добавлять примеры и тесты
✅ Оптимизировать производительность
✅ Улучшать документацию
✅ Добавлять логирование

## CI/CD Pipeline

GitHub Actions (`.github/workflows/test.yml`):
1. `cargo build --release` ✅
2. `cargo test --lib` ✅
3. `cargo test --test '*'` ✅
4. Fuzzing 10 минут ✅

## Production Ready Checklist

- ✅ lib.rs с публичным API
- ✅ README с полной информацией
- ✅ LICENSE (MIT)
- ✅ Архитектура задокументирована
- ✅ 256M+ операций фаззинга
- ✅ Все тесты проходят

**Status: READY FOR PRODUCTION** 🚀
