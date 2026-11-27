# corpus-audit-rename-move Specification

## Purpose

Проверка rename/move-сценариев phtools через etng.

## Requirements

### Requirement: Rename camera names
Аудит SHALL создавать копии с камерными именами и прогонять `phrename`.

#### Scenario: 17 supported files
- **WHEN** в корпусе 19 файлов, 2 CR2 не поддерживаются phtools
- **THEN** 17 файлов получают стандартные имена, CR2 остаются как есть

### Requirement: Move with categories
Аудит SHALL прогонять `phmove -a` и проверять раскладку.

#### Scenario: VIDEO/RAW
- **WHEN** перемещаются MOV/MTS/ARW/CR3
- **THEN** они оказываются в `VIDEO/` или `RAW/`, остальные — в корне
