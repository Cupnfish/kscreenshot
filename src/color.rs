type Matrix3 = [[f32; 3]; 3];

const MCAT02: Matrix3 = [
    [0.7328, 0.4296, -0.1624],
    [-0.7036, 1.6975, 0.0061],
    [0.0030, 0.0136, 0.9834],
];

pub fn srgb_primaries() -> [f32; 8] {
    [0.640, 0.330, 0.300, 0.600, 0.150, 0.060, 0.3127, 0.3290]
}

pub fn ctr_color_space(src: [f32; 8], dst: [f32; 8]) -> Option<Matrix3> {
    let src_white = tristimulus([src[6], src[7]], 1.0);
    let dst_white = tristimulus([dst[6], dst[7]], 1.0);

    let src_npm = npm(
        [[src[0], src[1]], [src[2], src[3]], [src[4], src[5]]],
        src_white,
    )?;
    let dst_npm = npm(
        [[dst[0], dst[1]], [dst[2], dst[3]], [dst[4], dst[5]]],
        dst_white,
    )?;

    let adapted_xyz = mul33(adapt_mat(MCAT02, src_white, dst_white)?, src_npm);
    let xyz_to_rgb = invert_matrix(dst_npm)?;

    Some(mul33(xyz_to_rgb, adapted_xyz))
}

pub fn apply_matrix(matrix: &Matrix3, rgb: [f32; 3]) -> [f32; 3] {
    mul3(*matrix, rgb)
}

fn tristimulus(xy: [f32; 2], y: f32) -> [f32; 3] {
    let z = 1.0 - xy[0] - xy[1];
    [y * xy[0] / xy[1], y, y * z / xy[1]]
}

fn adapt_mat(mat: Matrix3, source: [f32; 3], target: [f32; 3]) -> Option<Matrix3> {
    let w1 = mul3(mat, source);
    let w2 = mul3(mat, target);
    let q = [w2[0] / w1[0], w2[1] / w1[1], w2[2] / w1[2]];
    Some(mul33(mul3_col(invert_matrix(mat)?, q), mat))
}

fn invert_matrix(matrix: Matrix3) -> Option<Matrix3> {
    let det = matrix[0][0] * (matrix[1][1] * matrix[2][2] - matrix[1][2] * matrix[2][1])
        - matrix[0][1] * (matrix[1][0] * matrix[2][2] - matrix[1][2] * matrix[2][0])
        + matrix[0][2] * (matrix[1][0] * matrix[2][1] - matrix[1][1] * matrix[2][0]);

    if det.abs() < 1e-15 {
        return None;
    }

    let inv_det = 1.0 / det;
    Some([
        [
            (matrix[1][1] * matrix[2][2] - matrix[1][2] * matrix[2][1]) * inv_det,
            (matrix[0][2] * matrix[2][1] - matrix[0][1] * matrix[2][2]) * inv_det,
            (matrix[0][1] * matrix[1][2] - matrix[0][2] * matrix[1][1]) * inv_det,
        ],
        [
            (matrix[1][2] * matrix[2][0] - matrix[1][0] * matrix[2][2]) * inv_det,
            (matrix[0][0] * matrix[2][2] - matrix[0][2] * matrix[2][0]) * inv_det,
            (matrix[0][2] * matrix[1][0] - matrix[0][0] * matrix[1][2]) * inv_det,
        ],
        [
            (matrix[1][0] * matrix[2][1] - matrix[1][1] * matrix[2][0]) * inv_det,
            (matrix[0][1] * matrix[2][0] - matrix[0][0] * matrix[2][1]) * inv_det,
            (matrix[0][0] * matrix[1][1] - matrix[0][1] * matrix[1][0]) * inv_det,
        ],
    ])
}

fn mul3(matrix: Matrix3, vector: [f32; 3]) -> [f32; 3] {
    [
        matrix[0][0] * vector[0] + matrix[0][1] * vector[1] + matrix[0][2] * vector[2],
        matrix[1][0] * vector[0] + matrix[1][1] * vector[1] + matrix[1][2] * vector[2],
        matrix[2][0] * vector[0] + matrix[2][1] * vector[1] + matrix[2][2] * vector[2],
    ]
}

fn mul33(left: Matrix3, right: Matrix3) -> Matrix3 {
    let col0 = mul3(right, [1.0, 0.0, 0.0]);
    let col1 = mul3(right, [0.0, 1.0, 0.0]);
    let col2 = mul3(right, [0.0, 0.0, 1.0]);

    let mut result = [[0.0; 3]; 3];
    for row in 0..3 {
        let current = left[row];
        result[row][0] = current[0] * col0[0] + current[1] * col0[1] + current[2] * col0[2];
        result[row][1] = current[0] * col1[0] + current[1] * col1[1] + current[2] * col1[2];
        result[row][2] = current[0] * col2[0] + current[1] * col2[1] + current[2] * col2[2];
    }
    result
}

fn mul3_col(matrix: Matrix3, scale: [f32; 3]) -> Matrix3 {
    [
        [
            matrix[0][0] * scale[0],
            matrix[0][1] * scale[1],
            matrix[0][2] * scale[2],
        ],
        [
            matrix[1][0] * scale[0],
            matrix[1][1] * scale[1],
            matrix[1][2] * scale[2],
        ],
        [
            matrix[2][0] * scale[0],
            matrix[2][1] * scale[1],
            matrix[2][2] * scale[2],
        ],
    ]
}

fn npm(primaries: [[f32; 2]; 3], white: [f32; 3]) -> Option<Matrix3> {
    let x = [primaries[0][0], primaries[1][0], primaries[2][0]];
    let y = [primaries[0][1], primaries[1][1], primaries[2][1]];
    let z = [1.0 - x[0] - y[0], 1.0 - x[1] - y[1], 1.0 - x[2] - y[2]];

    let scale = solve3([x, y, z], white)?;

    Some([
        [x[0] * scale[0], x[1] * scale[1], x[2] * scale[2]],
        [y[0] * scale[0], y[1] * scale[1], y[2] * scale[2]],
        [z[0] * scale[0], z[1] * scale[1], z[2] * scale[2]],
    ])
}

fn solve3(matrix: Matrix3, rhs: [f32; 3]) -> Option<[f32; 3]> {
    let det = det3(matrix);
    if det.abs() <= 1e-15 {
        return None;
    }

    let inv_det = 1.0 / det;
    Some([
        det3(replace_column(matrix, rhs, 0)) * inv_det,
        det3(replace_column(matrix, rhs, 1)) * inv_det,
        det3(replace_column(matrix, rhs, 2)) * inv_det,
    ])
}

fn det3(matrix: Matrix3) -> f32 {
    matrix[0][0] * (matrix[1][1] * matrix[2][2] - matrix[1][2] * matrix[2][1])
        - matrix[0][1] * (matrix[1][0] * matrix[2][2] - matrix[1][2] * matrix[2][0])
        + matrix[0][2] * (matrix[1][0] * matrix[2][1] - matrix[1][1] * matrix[2][0])
}

fn replace_column(mut matrix: Matrix3, column: [f32; 3], column_index: usize) -> Matrix3 {
    matrix[0][column_index] = column[0];
    matrix[1][column_index] = column[1];
    matrix[2][column_index] = column[2];
    matrix
}

#[cfg(test)]
mod tests {
    use super::{ctr_color_space, srgb_primaries};

    #[test]
    fn same_color_space_returns_identity_like_matrix() {
        let src = srgb_primaries();
        let matrix = ctr_color_space(src, src).expect("matrix should be invertible");

        assert!((matrix[0][0] - 1.0).abs() < 1e-4);
        assert!((matrix[1][1] - 1.0).abs() < 1e-4);
        assert!((matrix[2][2] - 1.0).abs() < 1e-4);
    }
}
