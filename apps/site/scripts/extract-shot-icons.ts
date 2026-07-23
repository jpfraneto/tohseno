import {
  mkdir,
  readdir,
  rename,
  rm,
  unlink,
  writeFile,
} from "node:fs/promises";
import { join, resolve } from "node:path";

interface Cell {
  row: number;
  column: number;
}

interface SourceSheet {
  file: string;
  sha256: string;
  cropSize: number;
  x: readonly number[];
  y: readonly number[];
  cells: readonly Cell[];
}

interface GeneratedIcon {
  file: string;
  source: string;
  row: number;
  column: number;
}

const SOURCE_DIRECTORY = resolve(
  import.meta.dir,
  "../assets/shot-icon-sources",
);
const OUTPUT_DIRECTORY = resolve(import.meta.dir, "../public/shot-icons");
const STAGING_DIRECTORY = resolve(
  import.meta.dir,
  `../public/.shot-icons-generating-${process.pid}`,
);
const MANIFEST_PATH = resolve(
  import.meta.dir,
  "../assets/shot-icon-manifest.json",
);
const MANIFEST_STAGING_PATH = `${MANIFEST_PATH}.generating-${process.pid}`;
const OUTPUT_SIZE = 192;

const cells = (...pairs: ReadonlyArray<readonly [number, number]>): Cell[] =>
  pairs.map(([row, column]) => ({ row, column }));

const sheets: readonly SourceSheet[] = [
  {
    file: "01-practical-neon.png",
    sha256: "d3f17422ee203229adcb7e28e4bb79fe98bb89fa688a679aed19abebbaf6dfbb",
    cropSize: 158,
    x: [45, 245, 445, 645, 846, 1046],
    y: [42, 244, 446, 647, 849, 1050],
    cells: cells(
      [0, 0],
      [0, 1],
      [0, 2],
      [0, 3],
      [0, 4],
      [0, 5],
      [1, 0],
      [1, 1],
      [1, 2],
      [1, 4],
      [1, 5],
      [2, 0],
      [2, 1],
      [2, 2],
      [2, 3],
      [2, 4],
      [2, 5],
      [3, 0],
      [3, 1],
      [3, 2],
      [3, 3],
      [3, 4],
      [3, 5],
      [4, 1],
      [4, 2],
    ),
  },
  {
    file: "02-soft-cosmic.png",
    sha256: "b2760b6028308693f07213534f3a937b9334c1b639d8832102540f97f3a17e18",
    cropSize: 164,
    x: [42, 241, 440, 640, 840, 1042],
    y: [42, 236, 430, 625, 819, 1014],
    cells: cells(
      [0, 0],
      [0, 2],
      [0, 3],
      [0, 4],
      [0, 5],
      [1, 1],
      [1, 3],
      [1, 5],
      [2, 1],
      [2, 5],
      [3, 0],
      [3, 1],
      [3, 2],
      [3, 3],
      [3, 4],
      [4, 0],
      [4, 1],
      [4, 3],
      [4, 4],
      [4, 5],
      [5, 0],
      [5, 1],
      [5, 3],
      [5, 4],
      [5, 5],
    ),
  },
  {
    file: "03-psychedelic.png",
    sha256: "d32bcec8214af8d2acc47f23f81e31e3db93cd46f4e368fa431760a91addac51",
    cropSize: 171,
    x: [32, 232, 432, 632, 833, 1033],
    y: [32, 231, 430, 628, 827, 1026],
    cells: cells(
      [0, 0],
      [0, 1],
      [0, 3],
      [0, 4],
      [0, 5],
      [1, 0],
      [1, 1],
      [1, 2],
      [1, 3],
      [1, 4],
      [1, 5],
      [2, 0],
      [2, 1],
      [2, 2],
      [2, 3],
      [2, 4],
      [2, 5],
      [3, 0],
      [3, 1],
      [3, 2],
      [3, 3],
      [3, 4],
      [3, 5],
      [4, 0],
      [4, 5],
    ),
  },
  {
    file: "04-darkroom-raw.png",
    sha256: "ba4ba60077b29ab7737e1ed3b4878395bbefdadd77a419e7aed626d7502562c6",
    cropSize: 163,
    x: [48, 247, 447, 646, 846, 1045],
    y: [53, 248, 443, 638, 834, 1030],
    cells: cells(
      [0, 0],
      [0, 1],
      [0, 2],
      [0, 3],
      [0, 4],
      [0, 5],
      [1, 0],
      [1, 2],
      [1, 4],
      [1, 5],
      [2, 0],
      [2, 1],
      [2, 2],
      [2, 3],
      [2, 4],
      [2, 5],
      [3, 0],
      [3, 1],
      [3, 2],
      [3, 4],
      [3, 5],
      [4, 0],
      [4, 1],
      [4, 4],
      [4, 5],
    ),
  },
];

async function sha256(path: string): Promise<string> {
  const bytes = new Uint8Array(await Bun.file(path).arrayBuffer());
  return new Bun.CryptoHasher("sha256").update(bytes).digest("hex");
}

async function requireWebpTools(): Promise<void> {
  for (const tool of ["cwebp", "dwebp"]) {
    const process = Bun.spawn([tool, "-version"], {
      stdin: "ignore",
      stdout: "ignore",
      stderr: "pipe",
    });
    const errorText = new Response(process.stderr).text();
    if ((await process.exited) !== 0) {
      throw new Error(
        `${tool} is required to extract shot icons: ${(await errorText).trim()}`,
      );
    }
  }
}

async function prepareOutputDirectories(): Promise<void> {
  await rm(STAGING_DIRECTORY, { recursive: true, force: true });
  await mkdir(STAGING_DIRECTORY, { recursive: true });
  await mkdir(OUTPUT_DIRECTORY, { recursive: true });
}

async function extractIcon(
  sourcePath: string,
  outputPath: string,
  x: number,
  y: number,
  cropSize: number,
): Promise<void> {
  const process = Bun.spawn(
    [
      "cwebp",
      "-quiet",
      "-crop",
      String(x),
      String(y),
      String(cropSize),
      String(cropSize),
      "-resize",
      String(OUTPUT_SIZE),
      String(OUTPUT_SIZE),
      "-q",
      "82",
      "-m",
      "6",
      "-sharp_yuv",
      "-metadata",
      "none",
      sourcePath,
      "-o",
      outputPath,
    ],
    {
      stdin: "ignore",
      stdout: "ignore",
      stderr: "pipe",
    },
  );
  const errorText = new Response(process.stderr).text();
  if ((await process.exited) !== 0) {
    throw new Error(
      `cwebp failed for ${outputPath}: ${(await errorText).trim()}`,
    );
  }
}

async function main(): Promise<void> {
  await requireWebpTools();
  await prepareOutputDirectories();

  const generated: GeneratedIcon[] = [];
  let sequence = 1;

  for (const sheet of sheets) {
    const sourcePath = join(SOURCE_DIRECTORY, sheet.file);
    const actualHash = await sha256(sourcePath);
    if (actualHash !== sheet.sha256) {
      throw new Error(`Source sheet checksum mismatch for ${sheet.file}`);
    }

    for (const cell of sheet.cells) {
      const x = sheet.x[cell.column];
      const y = sheet.y[cell.row];
      if (x === undefined || y === undefined) {
        throw new Error(
          `Invalid cell ${cell.row + 1},${cell.column + 1} in ${sheet.file}`,
        );
      }

      const file = `shot-${String(sequence).padStart(3, "0")}.webp`;
      await extractIcon(
        sourcePath,
        join(STAGING_DIRECTORY, file),
        x,
        y,
        sheet.cropSize,
      );
      generated.push({
        file,
        source: sheet.file,
        row: cell.row + 1,
        column: cell.column + 1,
      });
      sequence += 1;
    }
  }

  if (generated.length !== 100) {
    throw new Error(`Expected 100 shot icons, generated ${generated.length}`);
  }

  const faviconProcess = Bun.spawn(
    [
      "dwebp",
      join(STAGING_DIRECTORY, "shot-076.webp"),
      "-o",
      join(STAGING_DIRECTORY, "favicon.png"),
    ],
    {
      stdin: "ignore",
      stdout: "ignore",
      stderr: "pipe",
    },
  );
  const faviconError = new Response(faviconProcess.stderr).text();
  if ((await faviconProcess.exited) !== 0) {
    throw new Error(
      `dwebp failed to create the favicon: ${(await faviconError).trim()}`,
    );
  }

  await writeFile(
    MANIFEST_STAGING_PATH,
    `${JSON.stringify(
      {
        generatedAt: "deterministic",
        output: {
          format: "webp",
          width: OUTPUT_SIZE,
          height: OUTPUT_SIZE,
          quality: 82,
          faviconSource: "shot-076.webp",
        },
        icons: generated,
      },
      null,
      2,
    )}\n`,
  );

  const generatedFiles = new Set(generated.map((icon) => icon.file));
  for (const icon of generated) {
    await rename(
      join(STAGING_DIRECTORY, icon.file),
      join(OUTPUT_DIRECTORY, icon.file),
    );
  }
  for (const entry of await readdir(OUTPUT_DIRECTORY, {
    withFileTypes: true,
  })) {
    if (
      entry.isFile() &&
      /^shot-\d{3}\.webp$/.test(entry.name) &&
      !generatedFiles.has(entry.name)
    ) {
      await unlink(join(OUTPUT_DIRECTORY, entry.name));
    }
  }
  await rename(
    join(STAGING_DIRECTORY, "favicon.png"),
    resolve(import.meta.dir, "../public/favicon.png"),
  );
  await rename(MANIFEST_STAGING_PATH, MANIFEST_PATH);
  await rm(STAGING_DIRECTORY, { recursive: true, force: true });

  console.info(
    `Generated ${generated.length} shot icons in ${OUTPUT_DIRECTORY}`,
  );
}

try {
  await main();
} finally {
  await rm(STAGING_DIRECTORY, { recursive: true, force: true });
  await rm(MANIFEST_STAGING_PATH, { force: true });
}
