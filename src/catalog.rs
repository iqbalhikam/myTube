use crate::model::Video;

fn slug(s: &str) -> String {
    s.to_lowercase().replace(' ', "-").replace('\'', "")
}

fn v(id: &str, title: &str, channel: &str, category: &str, duration: u32, views: u64, age: &str, color: [u8; 3]) -> Video {
    Video { id: id.into(), title: title.into(), channel: channel.into(), channel_id: slug(channel), category: category.into(), duration, views, age: age.into(), color, custom: false }
}

pub fn videos() -> Vec<Video> {
    vec![
        v("dQw4w9WgXcQ", "Rick Astley - Never Gonna Give You Up", "Rick Astley", "Musik", 213, 1_400_000_000, "14 tahun", [255, 45, 77]),
        v("jNQXAC9IVRw", "Me at the zoo", "jawed", "Vlog", 19, 330_000_000, "19 tahun", [46, 155, 255]),
        v("9bZkp7q19f0", "PSY - Gangnam Style", "officialpsy", "Musik", 253, 5_100_000_000, "13 tahun", [124, 92, 255]),
        v("kJQP7kiw5Fk", "Luis Fonsi - Despacito ft. Daddy Yankee", "Luis Fonsi", "Musik", 282, 8_500_000_000, "9 tahun", [255, 176, 32]),
        v("JGwWNGJdvx8", "Ed Sheeran - Shape of You", "Ed Sheeran", "Musik", 264, 6_200_000_000, "9 tahun", [34, 229, 132]),
        v("RgKAFK5djSk", "Wiz Khalifa - See You Again ft. Charlie Puth", "Wiz Khalifa", "Musik", 237, 6_300_000_000, "10 tahun", [25, 211, 218]),
        v("OPf0YbXqDm0", "Mark Ronson - Uptown Funk ft. Bruno Mars", "Mark Ronson", "Musik", 271, 5_200_000_000, "11 tahun", [255, 61, 174]),
        v("fJ9rUzIMcZQ", "Queen - Bohemian Rhapsody", "Queen Official", "Musik", 355, 1_800_000_000, "16 tahun", [255, 45, 77]),
        v("hT_nvWreIhg", "OneRepublic - Counting Stars", "OneRepublic", "Musik", 258, 4_200_000_000, "12 tahun", [46, 155, 255]),
        v("YQHsXMglC9A", "Adele - Hello", "Adele", "Musik", 367, 3_400_000_000, "10 tahun", [124, 92, 255]),
        v("CevxZvSJLk8", "Katy Perry - Roar", "Katy Perry", "Musik", 270, 4_000_000_000, "12 tahun", [255, 176, 32]),
        v("09R8_2nJtjg", "Maroon 5 - Sugar", "Maroon 5", "Musik", 302, 4_100_000_000, "11 tahun", [34, 229, 132]),
        v("lp-EO5I60KA", "Ed Sheeran - Thinking Out Loud", "Ed Sheeran", "Musik", 289, 3_800_000_000, "11 tahun", [25, 211, 218]),
        v("2Vv-BfVoq4g", "Ed Sheeran - Perfect", "Ed Sheeran", "Musik", 263, 3_700_000_000, "8 tahun", [255, 61, 174]),
        v("60ItHLz5WEA", "Alan Walker - Faded", "Alan Walker", "Musik", 213, 3_600_000_000, "10 tahun", [46, 155, 255]),
        v("7wtfhZwyrcc", "Imagine Dragons - Believer", "ImagineDragons", "Musik", 217, 2_500_000_000, "9 tahun", [255, 45, 77]),
        v("ktvTqknDobU", "Imagine Dragons - Radioactive", "ImagineDragons", "Musik", 245, 1_800_000_000, "13 tahun", [124, 92, 255]),
        v("e-ORhEE9VVg", "Taylor Swift - Blank Space", "Taylor Swift", "Musik", 273, 3_400_000_000, "11 tahun", [255, 176, 32]),
        v("nfWlot6h_JM", "Taylor Swift - Shake It Off", "Taylor Swift", "Musik", 242, 3_500_000_000, "11 tahun", [34, 229, 132]),
        v("450p7goxZqg", "John Legend - All of Me", "John Legend", "Musik", 300, 2_800_000_000, "12 tahun", [25, 211, 218]),
        v("kXYiU_JCYtU", "Linkin Park - Numb", "Linkin Park", "Musik", 187, 2_200_000_000, "17 tahun", [255, 61, 174]),
        v("1w7OgIMMRc4", "Guns N' Roses - Sweet Child O' Mine", "Guns N' Roses", "Musik", 303, 1_600_000_000, "16 tahun", [255, 45, 77]),
        v("QcIy9NiNbmo", "Taylor Swift - Bad Blood ft. Kendrick Lamar", "Taylor Swift", "Musik", 252, 1_500_000_000, "10 tahun", [46, 155, 255]),
        v("3AtDnEC4zak", "Charlie Puth - We Don't Talk Anymore ft. Selena Gomez", "Charlie Puth", "Musik", 230, 2_900_000_000, "9 tahun", [124, 92, 255]),
    ]
}
