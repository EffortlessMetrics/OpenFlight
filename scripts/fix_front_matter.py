import os
import yaml
import re
from pathlib import Path

DOCS_DIR = Path('docs')
SCHEMA_PATH = Path('schemas/front_matter.schema.json')

KIND_MAP = {
    'explanation': 'explanation',
    'how-to': 'how-to',
    'reference': 'reference',
    'tutorial': 'tutorial',
    'design': 'design',
    'concept': 'concept',
    'requirements': 'requirements',
    'adr': 'adr'
}

AREA_MAP = {
    'flight-core': 'flight-core',
    'flight-virtual': 'flight-virtual',
    'flight-hid': 'flight-hid',
    'flight-ipc': 'flight-ipc',
    'flight-scheduler': 'flight-scheduler',
    'flight-ffb': 'flight-ffb',
    'flight-panels': 'flight-panels',
    'infra': 'infra',
    'infrastructure': 'infra',
    'ci': 'ci',
    'simulation': 'simulation',
    'integration': 'integration',
    'ksp': 'ksp',
    'profile': 'profile'
}

def get_default_kind(path):
    parts = path.parts
    if 'explanation' in parts:
        if 'adr' in parts:
            return 'adr'
        return 'explanation'
    if 'how-to' in parts:
        return 'how-to'
    if 'reference' in parts:
        return 'reference'
    if 'tutorials' in parts:
        return 'tutorial'
    if 'design' in parts:
        return 'design'
    if 'dev' in parts:
        return 'explanation'
    return 'concept'

def get_default_area(path, content):
    # Try to infer area from content or path
    content_lower = content.lower()
    for area in AREA_MAP.values():
        if area in content_lower:
            return area
    return 'flight-core' # Default

def slugify(name):
    return re.sub(r'[^A-Z0-9]+', '-', name.upper()).strip('-')

def fix_file(path):
    with open(path, 'r', encoding='utf-8') as f:
        content = f.read()

    front_matter = {}
    body = content
    
    match = re.match(r'^---\s*\n(.*?)\n---\s*\n', content, re.DOTALL)
    if match:
        try:
            front_matter = yaml.safe_load(match.group(1))
            body = content[match.end():]
        except yaml.YAMLError:
            print(f"Error parsing YAML in {path}")
            return

    # Normalize fields
    if 'category' in front_matter:
        front_matter['kind'] = KIND_MAP.get(front_matter.pop('category'), 'concept')
    
    if 'group' in front_matter:
        front_matter['area'] = AREA_MAP.get(front_matter.pop('group'), 'flight-core')

    if 'kind' not in front_matter:
        front_matter['kind'] = get_default_kind(path)
    
    if 'area' not in front_matter:
        front_matter['area'] = get_default_area(path, content)

    if 'status' not in front_matter:
        front_matter['status'] = 'draft'
    
    if 'links' not in front_matter:
        front_matter['links'] = {}
    
    links = front_matter['links']
    if 'requirements' in front_matter:
        links['requirements'] = front_matter.pop('requirements')
    if 'tasks' in front_matter:
        links['tasks'] = front_matter.pop('tasks')
    if 'adrs' in front_matter:
        links['adrs'] = front_matter.pop('adrs')

    # Ensure requirements, tasks, adrs are arrays if they exist
    for k in ['requirements', 'tasks', 'adrs']:
        if k in links and not isinstance(links[k], list):
            links[k] = [links[k]]
        if k not in links:
            links[k] = []

    # Ensure doc_id
    # Always regenerate doc_id to ensure consistency and uniqueness across directories
    name_slug = slugify(path.stem)
    parent_parts = path.parent.relative_to(DOCS_DIR).parts
    if parent_parts:
        parent_slug = slugify("-".join(parent_parts))
        name_slug = f"{parent_slug}-{name_slug}"
    
    kind_slug = front_matter['kind'].upper()
    front_matter['doc_id'] = f"DOC-{kind_slug}-{name_slug}"

    # Remove extra fields
    allowed_fields = ['doc_id', 'kind', 'area', 'status', 'links']
    final_fm = {k: front_matter[k] for k in allowed_fields if k in front_matter}
    
    # Reorder fields for consistency
    ordered_fm = {}
    for k in allowed_fields:
        if k in final_fm:
            ordered_fm[k] = final_fm[k]
        else:
            # Should not happen given logic above
            pass

    new_content = "---\n" + yaml.dump(ordered_fm, sort_keys=False) + "---\n" + body
    
    with open(path, 'w', encoding='utf-8') as f:
        f.write(new_content)

if __name__ == '__main__':
    for md_file in DOCS_DIR.rglob('*.md'):
        print(f"Processing {md_file}")
        fix_file(md_file)
