use std::iter;
use std::error;
use std::fmt;
use std::fs::File;
use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::path::Path;
use std::num;

use draw;
use imagefmt;
use math::Vec3f;

type Vertex = Vec3f;

#[derive(Clone)]
struct FaceIndex {
    v_idxs: Vec<usize>,
    t_idxs: Vec<usize>,
    n_idxs: Vec<usize>,
}

impl FaceIndex {
    fn new() -> Self {
        FaceIndex {
            v_idxs: Vec::new(),
            t_idxs: Vec::new(),
            n_idxs: Vec::new(),
        }
    }
}

impl fmt::Display for FaceIndex {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f,
               "{} vert idx {} tex idx {} norm idx",
               self.v_idxs.len(),
               self.t_idxs.len(),
               self.n_idxs.len())
    }
}

// TODO(wathiede): rename 'Triangle'?
pub struct Face {
    pub vertices: [Vec3f; 3],
    pub texcoords: [Vec3f; 3],
}

impl fmt::Display for Face {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{:?} vertices", self.vertices)
    }
}

#[derive(Debug)]
pub enum Error {
    ParseError(ParseError),
    IoError(io::Error),
    ImagefmtError(imagefmt::Error),
}

impl From<ParseError> for Error {
    fn from(err: ParseError) -> Error {
        Error::ParseError(err)
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::IoError(err)
    }
}

impl From<imagefmt::Error> for Error {
    fn from(err: imagefmt::Error) -> Error {
        Error::ImagefmtError(err)
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::ParseError(ref err) => err.description(),
            Error::IoError(ref err) => err.description(),
            Error::ImagefmtError(ref err) => err.description(),
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            Error::ParseError(ref err) => Some(err),
            Error::IoError(ref err) => Some(err),
            Error::ImagefmtError(ref err) => Some(err),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match *self {
            Error::ParseError(ref e) => e.fmt(f),
            Error::IoError(ref e) => e.fmt(f),
            Error::ImagefmtError(ref e) => e.fmt(f),
        }
    }
}

#[derive(Debug)]
pub struct ParseError(String);

impl error::Error for ParseError {
    fn description(&self) -> &str {
        "Parse Error"
    }

    fn cause(&self) -> Option<&error::Error> {
        None
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "Parse Error: {}", self)
    }
}

impl From<num::ParseFloatError> for ParseError {
    fn from(err: num::ParseFloatError) -> ParseError {
        ParseError(format!("{}", err))
    }
}

impl From<num::ParseIntError> for ParseError {
    fn from(err: num::ParseIntError) -> ParseError {
        ParseError(format!("{}", err))
    }
}

pub struct Object {
    vertices: Vec<Vertex>,
    texcoords: Vec<Vertex>,
    faces: Vec<FaceIndex>,

    // TODO(wathiede): make this more flexible for multiple diffuse textures, and to support normal
    // and speculator maps.
    tex: draw::Texture2D,
}

impl Object {
    pub fn read<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let p = path.as_ref();
        let mut pb = p.to_path_buf();
        pb.set_file_name(p.file_stem().unwrap().to_string_lossy().to_string() + "_diffuse");
        pb.set_extension("tga");
        // TODO(wathiede): failure to load texture should be okay.
        let t = try!(draw::Texture2D::read(pb.as_path()));
        let mut obj = Object {
            vertices: Vec::new(),
            texcoords: Vec::new(),
            faces: Vec::new(),
            tex: t,
        };

        let f = BufReader::new(try!(File::open(p)));
        for (line_number, line) in f.lines().enumerate().map(|(a,b)| { (a+1, b) }) {
            let l = try!(line);
            try!(obj.parse_line(l).map_err(|e| { ParseError(format!("{} at line {}", e, line_number)) }));
        }
        Ok(obj)
    }

    pub fn vertex(&self, idx: usize) -> Vec3f {
        self.vertices[idx].clone()
    }

    pub fn texcoord(&self, idx: usize) -> Vec3f {
        self.texcoords[idx].clone()
    }

    // Samples the currently active texture map at uv. Performs nearest neighbor sampling.
    pub fn sample(&self, uv: Vec3f) -> draw::RGB {
        self.tex.sample(uv)
    }

    fn parse_line(&mut self, l: String) -> Result<(), ParseError> {
        let p: Vec<_> = l.split_whitespace().collect();
        if p.is_empty() {
            return Ok(());
        }
        match (p[0], &p[1..]) {
            ("#", _)  => { info!("Comment {:?}", l); Ok(()) },
            ("f", face) => self.add_face(face),
            ("v", vertex) => self.add_vertex(vertex),
            ("vn", _) => { debug!("Vertex normal {:?}", l); Ok(()) },
            ("vt", tex) => self.add_texcoord(tex),
            (t, _) => { info!("Unknown line type: {:?}", t); Ok(()) },
        }
    }

    fn add_face(&mut self, p: &[&str]) -> Result<(), ParseError> {
        debug!("Face {:?}", p);
        // TODO(wathiede): add support for quad faces, triangles only for now.
        if p.len() != 3 {
            return Err(ParseError("Bad vertex line".into()));
        }
        let mut f = FaceIndex::new();
        for n in p {
            for (vec, idx) in [&mut f.v_idxs, &mut f.t_idxs, &mut f.n_idxs].iter_mut().zip(n.split("/")) {
                vec.push(try!(idx.parse::<usize>()) - 1);
            }
        }
        self.faces.push(f);
        Ok(())
    }

    fn add_vertex(&mut self, p: &[&str]) -> Result<(), ParseError> {
        debug!("Vertex {:?}", p);
        // "v <x> <y> <z>"
        if p.len() != 3 {
            return Err(ParseError("Bad line".into()));
        };
        self.vertices.push(Vertex {
            x: try!(p[0].parse()),
            y: try!(p[1].parse()),
            z: try!(p[2].parse()),
        });
        Ok(())
    }

    fn add_texcoord(&mut self, p: &[&str]) -> Result<(), ParseError> {
        debug!("Texcoord {:?}", p);
        // "vt <x> <y> <z>"
        if p.len() != 3 {
            return Err(ParseError("Bad texcoord line".into()));
        };
        self.texcoords.push(Vertex {
            x: try!(p[0].parse()),
            y: try!(p[1].parse()),
            z: try!(p[2].parse())
        });
        Ok(())
    }
}

impl fmt::Display for Object {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f,
               "{} vertices {} faces",
               self.vertices.len(),
               self.faces.len())
    }
}

pub struct ObjectIter<'a> {
    obj: &'a Object,
    idx: usize,
}

impl<'a> iter::Iterator for ObjectIter<'a> {
    type Item = Face;
    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.obj.faces.len() {
            return None;
        }
        let ref f_idx = self.obj.faces[self.idx];
        // TODO(wathiede): add texcoord/normal values.
        let face = Face {
            vertices: [self.obj.vertex(f_idx.v_idxs[0]),
                       self.obj.vertex(f_idx.v_idxs[1]),
                       self.obj.vertex(f_idx.v_idxs[2])],
            texcoords: [self.obj.texcoord(f_idx.t_idxs[0]),
                        self.obj.texcoord(f_idx.t_idxs[1]),
                        self.obj.texcoord(f_idx.t_idxs[2])],
        };
        self.idx += 1;
        Some(face)
    }
}

impl<'a> iter::IntoIterator for &'a Object {
    type Item = Face;
    type IntoIter = ObjectIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        ObjectIter {
            obj: self,
            idx: 0,
        }
    }
}
