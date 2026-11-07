"""Independent FIT decoding check. Run after cargo test with venv/bin/python."""
import json
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
from fit_tool.fit_file import FitFile

ROOT = Path(__file__).resolve().parents[2]
RUSTC = shutil.which('rustc')
if RUSTC is None and (Path.home()/'.cargo/bin/rustc').is_file():
    RUSTC = str(Path.home()/'.cargo/bin/rustc')
if RUSTC is None:
    raise RuntimeError('rustc is not on PATH. Activate the Rust toolchain before running this check.')

def crc(data):
    value = 0
    for byte in data:
        value ^= byte
        for _ in range(8):
            value = (value >> 1) ^ (0xA001 if value & 1 else 0)
    return value

def fixture(big):
    order = '>' if big else '<'
    body = bytearray()
    def msg(global_id, fields, values):
        body.extend(bytes([64, 0, big]) + struct.pack(order+'H', global_id) + bytes([len(fields)]))
        for field in fields:
            body.extend(bytes(field))
        body.append(0)
        for (_, size, base), value in zip(fields, values):
            body.extend(value if isinstance(value, bytes) else int(value).to_bytes(size, 'big' if big else 'little'))
    msg(0, [(0,1,0),(1,2,132),(2,2,132),(3,4,140),(8,12,7)], [4,331,3570,3313379353,b'MyWhoosh\0\0\0\0'])
    fields = [(0,1,2),(2,2,132),(3,4,140),(4,2,132),(5,2,132),(19,12,7),(27,12,7)]
    msg(23, fields, [0,331,3313379353,3570,0,b'Old recorder',b'MyWhoosh\0\0\0\0'])
    # An accessory sharing the primary recorder's definition.
    body.extend(bytes([0,1])+struct.pack(order+'HIHH',32,778899,77,123)+b'HR sensor\0\0\0'+b'Accessory\0\0\0')
    msg(49,[(0,2,132),(1,1,2)],[0,9])
    msg(72,[(1,2,132),(2,2,132),(3,4,140)],[331,3570,3313379353])
    msg(20,[(253,4,134),(3,1,2),(4,1,2),(7,2,132)],[1100000000,145,80,220])
    result=bytearray([12,32,0,0])+struct.pack('<I',len(body))+b'.FIT'+body
    result.extend(struct.pack('<H',crc(result)))
    return result

def decode(path):
    return [(r.message.name, {f.name:f.get_value() for f in r.message.fields if f.get_value() is not None})
            for r in FitFile.from_file(str(path)).records if hasattr(r.message,'name')]

with tempfile.TemporaryDirectory() as directory:
    temp = Path(directory)
    harness = temp/'check.rs'
    harness.write_text(f'''#[path={json.dumps(str(ROOT/'desktop/src/fit.rs'))}] mod fit;
#[path={json.dumps(str(ROOT/'desktop/src/devices.rs'))}] mod devices;
fn main() {{ let a: Vec<String> = std::env::args().collect();
let input=std::fs::read(&a[1]).unwrap();
let output=fit::modify(&input,a[3].parse().unwrap(),Some(123456789)).unwrap();
std::fs::write(&a[2],output).unwrap(); }}''')
    deps=ROOT/'desktop/target/debug/deps'
    anyhow=next(deps.glob('libanyhow-*.rlib'))
    subprocess.run([RUSTC,'--edition=2021','-Awarnings',str(harness),'-L',str(deps),'--extern',f'anyhow={anyhow}','-o',str(temp/'check')],check=True)
    audit=json.loads((ROOT/'desktop/tests/device_catalog_audit.json').read_text())
    for big in [False, True]:
        original=temp/'original.fit'; original.write_bytes(fixture(big))
        before=decode(original)
        for profile in audit['edge_firmware']:
            product=profile['product']; firmware=profile['software_raw']
            name=next(d['name'] for d in audit['devices'] if d['product']==product)
            output=temp/'out.fit'
            subprocess.run([str(temp/'check'),str(original),str(output),str(product)],check=True)
            assert crc(output.read_bytes())==0
            after=decode(output)
            sensors=lambda rows:[v for n,v in rows if n=='device_info' and v.get('device_index')==1]
            assert sensors(after)==sensors(before)
            assert [v for n,v in after if n=='record']==[v for n,v in before if n=='record']
            for kind, values in after:
                if kind in ('file_id','training_file') or (kind=='device_info' and values.get('device_index')==0):
                    assert values['manufacturer']==1, (kind,values)
                    assert values['product']==product, (kind,values)
                    assert values['serial_number']==123456789
                    if kind!='training_file': assert values['product_name']==name
                if kind=='device_info' and values.get('device_index')==0:
                    assert values['descriptor']==name
                    assert abs(values['software_version']-firmware/100)<0.0001
                if kind=='file_creator': assert values['software_version']==firmware
    print('24 FIT exports decoded independently: 12 Edge models, both byte orders; identity, firmware, sensors, samples and CRC verified.')
